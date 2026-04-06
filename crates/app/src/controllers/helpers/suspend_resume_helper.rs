// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/suspend_resume_helper.go

pub struct SuspendResumeHelper {
    common: HelperCommon,
}

pub struct HelperCommon;

impl SuspendResumeHelper {
    pub fn new(common: HelperCommon) -> Self {
        Self { common }
    }

    pub fn can_suspend_app(&self) -> bool {
        crate::controllers::helpers::signal_handling::can_suspend_app()
    }

    pub fn suspend_app(&self) -> Result<(), String> {
        if !self.can_suspend_app() {
            return Ok(());
        }
        Ok(())
    }

    pub fn install_resume_signal_handler(&self) {}
}
