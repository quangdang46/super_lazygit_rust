// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/view_helper.go

pub struct ViewHelper {
    common: HelperCommon,
}

pub struct HelperCommon;
pub struct ContextTree;

impl ViewHelper {
    pub fn new(common: HelperCommon, _contexts: &ContextTree) -> Self {
        Self { common }
    }

    pub fn context_for_view(&self, _view_name: &str) -> Option<Context> {
        None
    }
}

pub struct Context;
pub struct View;
