// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/amend_helper.go

pub struct GpgHelper;

pub struct HelperCommon;

pub struct AmendHelper {
    context: HelperCommon,
    gpg: GpgHelper,
}

impl AmendHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
            gpg: GpgHelper,
        }
    }

    pub fn amend_head(&self) -> Result<(), String> {
        Ok(())
    }
}

impl Default for AmendHelper {
    fn default() -> Self {
        Self::new()
    }
}
