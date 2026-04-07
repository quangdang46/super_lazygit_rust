// Ported from ./references/lazygit-master/pkg/gui/controllers/reflog_commits_controller.go
use crate::controllers::ControllerCommon;

pub struct ReflogCommitsController {
    common: ControllerCommon,
}

pub struct ListControllerTrait;

impl ReflogCommitsController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn context(&self) -> ReflogCommitsContext {
        ReflogCommitsContext
    }

    pub fn get_on_render_to_main(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }
}

pub struct ReflogCommitsContext;
