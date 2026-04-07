// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/bisect_helper.go

pub struct HelperCommon;

pub struct BisectHelper {
    context: HelperCommon,
}

impl BisectHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
        }
    }

    pub fn reset(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn post_bisect_command_refresh(&self) {}
}

impl Default for BisectHelper {
    fn default() -> Self {
        Self::new()
    }
}
