// Ported from ./references/lazygit-master/pkg/gui/gui_driver.go

use crate::gui::gui::Gui;

pub struct GuiDriver {
    gui: Gui,
}

impl GuiDriver {
    pub fn press_key(&self, _key_str: String) {}

    pub fn click(&self, _x: i32, _y: i32) {}

    pub fn check_all_toasts_acknowledged(&self) {}

    pub fn keys(&self) -> String {
        String::new()
    }

    pub fn current_context(&self) -> String {
        String::new()
    }

    pub fn context_for_view(&self, _view_name: String) -> String {
        String::new()
    }

    pub fn fail(&self, _message: String) {
        panic!("GuiDriver fail called")
    }

    pub fn log(&self, _message: String) {}

    pub fn log_ui(&self, _message: String) {}

    pub fn checked_out_ref(&self) -> String {
        String::new()
    }

    pub fn main_view(&self) -> String {
        String::new()
    }

    pub fn secondary_view(&self) -> String {
        String::new()
    }

    pub fn view(&self, _view_name: String) -> String {
        String::new()
    }

    pub fn set_caption(&self, _caption: String) {}

    pub fn set_caption_prefix(&self, _prefix: String) {}

    pub fn next_toast(&self) -> Option<String> {
        None
    }

    pub fn headless(&self) -> bool {
        false
    }
}
