// Ported from ./references/lazygit-master/pkg/gui/controllers/confirmation_controller.go

use crate::context::confirmation_context::ConfirmationContext;
use crate::controllers::ControllerCommon;
use crate::types::context::{Binding, KeybindingsOpts, ViewMouseBinding, ViewMouseBindingOpts};

pub struct ConfirmationController {
    common: ControllerCommon,
    confirmation_context: ConfirmationContext,
}

pub struct OnFocusOpts;
pub struct OnFocusLostOpts;

impl ConfirmationController {
    pub fn new(common: ControllerCommon) -> Self {
        Self {
            common,
            confirmation_context: ConfirmationContext::new(crate::types::common::ContextCommon),
        }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        vec![]
    }

    pub fn get_mouse_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<ViewMouseBinding> {
        vec![]
    }

    pub fn get_on_double_click(&self) -> Option<Box<dyn Fn() -> Result<(), String>>> {
        None
    }

    pub fn get_on_click(&self) -> Option<Box<dyn Fn(ViewMouseBindingOpts) -> Result<(), String>>> {
        None
    }

    pub fn context(&self) -> &ConfirmationContext {
        &self.confirmation_context
    }

    pub fn get_on_focus_lost(&self) -> Option<Box<dyn Fn(OnFocusLostOpts)>> {
        Some(Box::new(|_opts: OnFocusLostOpts| {
            // self.c.Helpers().Confirmation.DeactivateConfirmation()
        }))
    }

    pub fn get_on_focus(&self) -> Option<Box<dyn Fn(OnFocusOpts)>> {
        None
    }

    pub fn get_on_render_to_main(&self) -> Option<Box<dyn Fn()>> {
        None
    }

    pub fn handle_copy_to_clipboard(&self) -> Result<(), String> {
        // confirmationView := self.c.Views().Confirmation
        // text := confirmationView.Buffer()
        // if err := self.c.OS().CopyToClipboard(text); err != nil {
        //     return err
        // }
        // self.c.Toast(self.c.Tr.MessageCopiedToClipboard)
        Ok(())
    }
}
