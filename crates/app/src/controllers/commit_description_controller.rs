use crate::controllers::ControllerCommon;

pub struct CommitDescriptionController {
    common: ControllerCommon,
}

impl CommitDescriptionController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }
    pub fn get_keybindings(&self) -> Vec<Binding> {
        Vec::new()
    }
    pub fn handle_toggle_panel(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct Binding {
    pub key: String,
    pub description: String,
}
