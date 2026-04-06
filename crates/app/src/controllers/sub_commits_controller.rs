// Ported from ./references/lazygit-master/pkg/gui/controllers/sub_commits_controller.go

pub struct SubCommitsController {
    common: ControllerCommon,
    list_controller_trait: ListControllerTrait<Commit>,
}

pub struct Commit;

impl SubCommitsController {
    pub fn new(common: ControllerCommon) -> Self {
        Self {
            common,
            list_controller_trait: ListControllerTrait::new(),
        }
    }

    pub fn context(&self) -> String {
        String::new()
    }

    fn context_ref(&self) -> SubCommitsContext {
        SubCommitsContext
    }

    pub fn get_on_render_to_main(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }

    pub fn get_on_focus(&self) -> Box<dyn Fn(OnFocusOpts)> {
        Box::new(|_opts| {})
    }
}

pub struct ListControllerTrait<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T> ListControllerTrait<T> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

pub struct SubCommitsContext;
pub struct ControllerCommon;
pub struct OnFocusOpts;
