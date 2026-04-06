// Ported from ./references/lazygit-master/pkg/gui/controllers/stash_controller.go

pub struct StashController {
    common: ControllerCommon,
    list_controller_trait: ListControllerTrait<StashEntry>,
}

pub struct StashEntry {
    pub index: usize,
    pub hash: String,
    pub description: String,
}

impl StashEntry {
    pub fn full_ref_name(&self) -> String {
        String::new()
    }
    pub fn ref_name(&self) -> String {
        String::new()
    }
    pub fn name(&self) -> String {
        String::new()
    }
}

impl StashController {
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

    fn context(&self) -> StashContext {
        StashContext
    }

    pub fn handle_stash_apply(&self, _stash_entry: &StashEntry) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_stash_pop(&self, _stash_entry: &StashEntry) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_stash_drop(&self, _stash_entries: &[StashEntry]) -> Result<(), String> {
        Ok(())
    }

    fn post_stash_refresh(&self) {}

    pub fn handle_new_branch_off_stash_entry(
        &self,
        _stash_entry: &StashEntry,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_rename_stash_entry(&self, _stash_entry: &StashEntry) -> Result<(), String> {
        Ok(())
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

pub struct StashContext;
pub struct ControllerCommon;
pub struct KeybindingsOpts;
pub struct Binding {
    pub key: char,
    pub description: String,
}
pub struct ConfirmOpts;
pub struct PromptOpts;
