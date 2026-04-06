use crate::controllers::ControllerCommon;

pub struct CommitMessageController {
    common: ControllerCommon,
}

impl CommitMessageController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn get_keybindings(&self) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_mouse_keybindings(&self) -> Vec<MouseBinding> {
        Vec::new()
    }

    pub fn handle_previous_commit(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_next_commit(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn switch_to_commit_description(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_toggle_panel(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn confirm(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn close_panel(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn open_commit_menu(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn on_click(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct Binding {
    pub key: String,
    pub description: String,
}

pub struct MouseBinding {
    pub view_name: String,
    pub focused_view: String,
}
