use crate::controllers::ControllerCommon;

pub struct CustomPatchOptionsMenuActionController {
    common: ControllerCommon,
}

impl CustomPatchOptionsMenuActionController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }
    pub fn get_keybindings(&self) -> Vec<Binding> {
        Vec::new()
    }
}

pub struct Binding {
    pub key: String,
    pub description: String,
}
