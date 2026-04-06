// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/bisect_helper.go

pub struct BisectHelper {
    context: HelperCommon,
}

pub struct HelperCommon;

impl BisectHelper {
    pub fn new(context: HelperCommon) -> Self {
        Self { context }
    }

    pub fn reset(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn post_bisect_command_refresh(&self) {}
}

impl BisectHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
        }
    }
}

impl Default for BisectHelper {
    fn default() -> Self {
        Self::new()
    }
}
