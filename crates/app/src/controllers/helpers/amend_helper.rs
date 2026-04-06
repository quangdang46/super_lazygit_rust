// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/amend_helper.go

pub struct AmendHelper {
    context: HelperCommon,
    gpg: GpgHelper,
}

pub struct HelperCommon;

pub struct GpgHelper;

impl AmendHelper {
    pub fn new(context: HelperCommon, gpg: GpgHelper) -> Self {
        Self { context, gpg }
    }

    pub fn amend_head(&self) -> Result<(), String> {
        Ok(())
    }
}

impl AmendHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
            gpg: GpgHelper,
        }
    }
}

impl Default for AmendHelper {
    fn default() -> Self {
        Self::new()
    }
}
