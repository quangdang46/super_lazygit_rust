// Ported from ./references/lazygit-master/pkg/gui/controllers/worktrees_controller.go
use crate::controllers::ControllerCommon;

pub struct WorktreesController {
    common: ControllerCommon,
    list_controller_trait: ListControllerTrait<Worktree>,
}

pub struct Worktree {
    pub name: String,
    pub branch: String,
    pub head: String,
    pub path: String,
    pub is_main: bool,
    pub is_path_missing: bool,
    pub is_current: bool,
}

impl WorktreesController {
    pub fn new(common: ControllerCommon) -> Self {
        Self {
            common,
            list_controller_trait: ListControllerTrait::new(),
        }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_on_render_to_main(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }

    pub fn add(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn remove(&self, _worktree: &Worktree) -> Result<(), String> {
        Ok(())
    }

    pub fn get_on_double_click(&self) -> Box<dyn Fn() -> Result<(), String>> {
        Box::new(|| Ok(()))
    }

    pub fn enter(&self, _worktree: &Worktree) -> Result<(), String> {
        Ok(())
    }

    pub fn open(&self, _worktree: &Worktree) -> Result<(), String> {
        Ok(())
    }

    fn context(&self) -> WorktreesContext {
        WorktreesContext
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

pub struct WorktreesContext;
pub struct KeybindingsOpts;
pub struct Binding {
    pub key: char,
    pub description: String,
}
