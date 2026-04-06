// Ported from ./references/lazygit-master/pkg/gui/gui_common.go

pub struct GuiCommon;

impl GuiCommon {
    pub fn log_action(&self, _msg: String) {}

    pub fn log_command(&self, _cmd_str: String, _is_command_line: bool) {}

    pub fn refresh(&self, _opts: RefreshOptions) {}

    pub fn post_refresh_update(&self, _context: String) {}

    pub fn run_subprocess_and_refresh(&self, _cmd_obj: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn run_subprocess(&self, _cmd_obj: &str) -> (bool, Result<(), String>) {
        (false, Ok(()))
    }

    pub fn suspend(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn resume(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn context(&self) -> String {
        String::new()
    }

    pub fn context_for_key(&self, _key: String) -> String {
        String::new()
    }

    pub fn get_app_state(&self) -> String {
        String::new()
    }

    pub fn save_app_state(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn get_config(&self) -> String {
        String::new()
    }

    pub fn reset_view_origin(&self, _view: &str) {}

    pub fn set_view_content(&self, _view: &str, _content: String) {}

    pub fn render(&self) {}

    pub fn views(&self) -> String {
        String::new()
    }

    pub fn git(&self) -> String {
        String::new()
    }

    pub fn os(&self) -> String {
        String::new()
    }

    pub fn modes(&self) -> String {
        String::new()
    }

    pub fn model(&self) -> String {
        String::new()
    }
}

pub struct RefreshOptions;
