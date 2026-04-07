// Ported from ./references/lazygit-master/pkg/gui/controllers/staging_controller.go
use crate::controllers::ControllerCommon;

pub struct StagingController {
    common: ControllerCommon,
    context: Box<dyn IPatchExplorerContext>,
    other_context: Box<dyn IPatchExplorerContext>,
    staged: bool,
}

pub trait IPatchExplorerContext {
    fn get_key(&self) -> String;
    fn get_mutex(&self) -> Mutex;
    fn get_state(&self) -> Option<PatchExplorerState>;
    fn set_state(&self, state: Option<PatchExplorerState>);
    fn get_view_name(&self) -> String;
}

pub struct PatchExplorerState;
pub struct Mutex;

impl Mutex {
    pub fn lock(&self) {}
    pub fn unlock(&self) {}
}

impl PatchExplorerState {
    pub fn current_line_number(&self) -> i32 {
        0
    }
    pub fn selecting_range(&self) -> bool {
        false
    }
    pub fn selecting_hunk_enabled_by_user(&self) -> bool {
        false
    }
    pub fn set_line_select_mode(&self) {}
    pub fn selected_patch_range(&self) -> (usize, usize) {
        (0, 0)
    }
    pub fn get_diff(&self) -> String {
        String::new()
    }
    pub fn selected_view_range(&self) -> (usize, usize) {
        (0, 0)
    }
    pub fn select_line(&self, _line: usize) {}
    pub fn current_hunk_bounds(&self) -> (usize, usize) {
        (0, 0)
    }
    pub fn get_selected_patch_line_idx(&self) -> usize {
        0
    }
}

impl StagingController {
    pub fn new(
        common: ControllerCommon,
        context: Box<dyn IPatchExplorerContext>,
        other_context: Box<dyn IPatchExplorerContext>,
        staged: bool,
    ) -> Self {
        Self {
            common,
            context,
            other_context,
            staged,
        }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn context(&self) -> String {
        self.context.get_key()
    }

    pub fn get_mouse_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<ViewMouseBinding> {
        Vec::new()
    }

    pub fn get_on_focus(&self) -> Box<dyn Fn(OnFocusOpts)> {
        Box::new(|_opts| {})
    }

    pub fn get_on_focus_lost(&self) -> Box<dyn Fn(OnFocusLostOpts)> {
        Box::new(|_opts| {})
    }

    pub fn open_file(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn edit_file(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn escape(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn escape_description(&self) -> String {
        String::new()
    }

    pub fn toggle_panel(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn toggle_staged(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn discard_selection(&self) -> Result<(), String> {
        Ok(())
    }

    fn apply_selection_and_refresh(&self, reverse: bool) -> Result<(), String> {
        self.apply_selection(reverse)?;
        Ok(())
    }

    fn apply_selection(&self, _reverse: bool) -> Result<(), String> {
        Ok(())
    }

    pub fn edit_hunk_and_refresh(&self) -> Result<(), String> {
        self.edit_hunk()?;
        Ok(())
    }

    fn edit_hunk(&self) -> Result<(), String> {
        Ok(())
    }

    fn file_path(&self) -> String {
        String::new()
    }
}

pub struct KeybindingsOpts;
pub struct OnFocusOpts;
pub struct OnFocusLostOpts;
pub struct ViewMouseBinding;
pub struct Binding {
    pub key: char,
    pub handler: Box<dyn Fn() -> Result<(), String>>,
    pub description: String,
}
pub struct ConfirmOpts;
