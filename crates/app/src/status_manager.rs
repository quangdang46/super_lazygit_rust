// Ported from ./references/lazygit-master/pkg/gui/status/status_manager.go

use ratatui::style::Color;
use std::sync::Mutex;

use crate::controllers::helpers::app_status_helper::ToastKind;

pub struct StatusManager {
    statuses: Vec<AppStatus>,
    next_id: i32,
    mutex: Mutex<()>,
}

struct WaitingStatusHandle<'a> {
    status_manager: &'a StatusManager,
    message: String,
    render_func: Box<dyn Fn()>,
    id: i32,
}

impl<'a> WaitingStatusHandle<'a> {
    fn show(&mut self) {
        self.id = self
            .status_manager
            .add_status(&self.message, "waiting", ToastKind::Status);
        (self.render_func)();
    }

    fn hide(&self) {
        self.status_manager.remove_status(self.id);
    }
}

pub struct AppStatus {
    message: String,
    status_type: String,
    color: Color,
    id: i32,
}

impl StatusManager {
    pub fn new() -> Self {
        Self {
            statuses: Vec::new(),
            next_id: 0,
            mutex: Mutex::new(()),
        }
    }

    pub fn with_waiting_status<F>(
        &self,
        message: &str,
        render_func: Box<dyn Fn()>,
        f: F,
    ) -> Result<(), String>
    where
        F: FnOnce(&mut WaitingStatusHandle) -> Result<(), String>,
    {
        let mut handle = WaitingStatusHandle {
            status_manager: self,
            message: message.to_string(),
            render_func,
            id: -1,
        };
        handle.show();
        handle.hide();
        f(&mut handle)
    }

    pub fn add_toast_status(&self, message: &str, kind: ToastKind) -> i32 {
        let id = self.add_status(message, "toast", kind);
        id
    }

    pub fn get_status_string(&self) -> (String, Color) {
        let guard = self.mutex.lock().unwrap();
        if self.statuses.is_empty() {
            return (String::new(), Color::Reset);
        }
        let top_status = &self.statuses[0];
        if top_status.status_type == "waiting" {
            let spinner = Self::get_spinner();
            return (
                format!("{} {}", top_status.message, spinner),
                top_status.color,
            );
        }
        (top_status.message.clone(), top_status.color)
    }

    pub fn has_status(&self) -> bool {
        let guard = self.mutex.lock().unwrap();
        !self.statuses.is_empty()
    }

    fn add_status(&self, message: &str, status_type: &str, kind: ToastKind) -> i32 {
        let mut guard = self.mutex.lock().unwrap();
        self.next_id += 1;
        let id = self.next_id;

        let color = match kind {
            ToastKind::Error => Color::Red,
            ToastKind::Status => Color::Cyan,
        };

        let new_status = AppStatus {
            message: message.to_string(),
            status_type: status_type.to_string(),
            color,
            id,
        };

        self.statuses.insert(0, new_status);
        id
    }

    fn remove_status(&self, id: i32) {
        let mut guard = self.mutex.lock().unwrap();
        self.statuses.retain(|status| status.id != id);
    }

    fn get_spinner() -> &'static str {
        let spinners = ["/", "-", "\\", "|"];
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let idx = (now % 4) as usize;
        spinners[idx]
    }
}

impl Default for StatusManager {
    fn default() -> Self {
        Self::new()
    }
}
