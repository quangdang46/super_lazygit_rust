// Ported from ./references/lazygit-master/pkg/gui/controllers/patch_explorer_controller.go
use crate::controllers::ControllerCommon;

pub struct PatchExplorerControllerFactory {
    common: ControllerCommon,
}

pub struct PatchExplorerController {
    common: ControllerCommon,
    context: PatchExplorerContext,
}

#[derive(Clone)]
pub struct PatchExplorerContext;
pub struct ViewMouseBinding;
pub struct Binding;
pub struct KeybindingsOpts;

impl PatchExplorerControllerFactory {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn create(&self, context: PatchExplorerContext) -> PatchExplorerController {
        PatchExplorerController {
            common: self.common.clone(),
            context,
        }
    }
}

impl PatchExplorerController {
    pub fn context(&self) -> PatchExplorerContext {
        self.context.clone()
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_mouse_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<ViewMouseBinding> {
        Vec::new()
    }

    pub fn handle_prev_line(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_next_line(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_prev_line_range(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_next_line_range(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_prev_hunk(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_next_hunk(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_toggle_select_range(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_toggle_select_hunk(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_scroll_left(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_scroll_right(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_prev_page(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_next_page(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_goto_top(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_goto_bottom(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_mouse_down(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_mouse_drag(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn copy_selected_to_clipboard(&self) -> Result<(), String> {
        Ok(())
    }
}
