// Ported from ./references/lazygit-master/pkg/gui/controllers/merge_conflicts_controller.go

pub struct MergeConflictsController {
    common: ControllerCommon,
}

pub struct ControllerCommon;
pub struct MergeConflictsContext;
pub struct ViewMouseBinding {
    pub view_name: String,
    pub key: char,
}
pub struct Binding {
    pub key: char,
    pub handler: Box<dyn Fn() -> Result<(), String>>,
    pub description: String,
}
pub struct KeybindingsOpts;
pub struct OnFocusOpts;
pub struct OnFocusLostOpts;

impl MergeConflictsController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_mouse_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<ViewMouseBinding> {
        Vec::new()
    }

    pub fn get_on_focus(&self) -> Box<dyn Fn(OnFocusOpts)> {
        Box::new(|_| {})
    }

    pub fn get_on_focus_lost(&self) -> Box<dyn Fn(OnFocusLostOpts)> {
        Box::new(|_| {})
    }

    pub fn handle_scroll_up(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_scroll_down(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn context(&self) -> MergeConflictsContext {
        MergeConflictsContext
    }

    pub fn escape(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_edit_file(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_open_file(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_scroll_left(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_scroll_right(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_undo(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn prev_conflict_hunk(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn next_conflict_hunk(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn next_conflict(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn prev_conflict(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_pick_hunk(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_pick_all_hunks(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn open_merge_conflict_menu(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct ViewMouseBindingOpts;
