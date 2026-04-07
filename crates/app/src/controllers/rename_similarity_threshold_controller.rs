// Ported from ./references/lazygit-master/pkg/gui/controllers/rename_similarity_threshold_controller.go
use crate::controllers::ControllerCommon;

pub struct RenameSimilarityThresholdController {
    common: ControllerCommon,
}

impl RenameSimilarityThresholdController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn context(&self) -> Option<Context> {
        None
    }

    pub fn increase(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn decrease(&self) -> Result<(), String> {
        Ok(())
    }

    fn apply_change(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct KeybindingsOpts;
pub struct Binding;
pub struct Context;
