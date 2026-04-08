// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/app_status_helper.go

pub struct AppStatusHelper {
    context: HelperCommon,
    status_mgr: fn() -> Box<dyn StatusManager>,
    mode_helper: ModeHelper,
}

pub struct HelperCommon;

pub struct ModeHelper;

pub trait StatusManager {
    fn add_toast_status(&self, message: &str, kind: ToastKind);
    fn has_status(&self) -> bool;
    fn get_status_string(&self);
}

pub enum ToastKind {
    Status,
    Error,
}

impl AppStatusHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
            status_mgr: || Box::new(MockStatusManager),
            mode_helper: ModeHelper,
        }
    }

    pub fn toast(&self, _message: &str, _kind: ToastKind) {}

    pub fn with_waiting_status<F>(&self, _message: &str, _f: F)
    where
        F: Fn() -> Result<(), String>,
    {
    }

    pub fn with_waiting_status_impl<F>(&self, _message: &str, _f: F)
    where
        F: Fn() -> Result<(), String>,
    {
    }

    pub fn with_waiting_status_sync<F>(&self, _message: &str, _f: F) -> Result<(), String>
    where
        F: Fn() -> Result<(), String>,
    {
        Ok(())
    }

    pub fn has_status(&self) -> bool {
        false
    }

    pub fn get_status_string(&self) -> String {
        String::new()
    }

    fn render_app_status(&self) {}

    fn render_app_status_sync(&self, _stop: std::sync::mpsc::Receiver<()>) {}
}

impl Default for AppStatusHelper {
    fn default() -> Self {
        Self::new()
    }
}

struct MockStatusManager;

impl StatusManager for MockStatusManager {
    fn add_toast_status(&self, _message: &str, _kind: ToastKind) {}
    fn has_status(&self) -> bool {
        false
    }
    fn get_status_string(&self) {}
}
