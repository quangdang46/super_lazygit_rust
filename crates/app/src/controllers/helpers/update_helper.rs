// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/update_helper.go

pub struct UpdateHelper {
    common: HelperCommon,
    updater: Updater,
}

pub struct HelperCommon;
pub struct Updater;

impl UpdateHelper {
    pub fn new(common: HelperCommon, updater: Updater) -> Self {
        Self { common, updater }
    }

    pub fn check_for_update_in_background(&self) {}

    pub fn check_for_update_in_foreground(&self) -> Result<(), String> {
        Ok(())
    }

    fn start_updating(&self, _new_version: &str) {}

    fn on_update_finish(&self, _err: Option<String>) -> Result<(), String> {
        Ok(())
    }

    fn show_update_prompt(&self, _new_version: &str) -> Result<(), String> {
        Ok(())
    }
}
