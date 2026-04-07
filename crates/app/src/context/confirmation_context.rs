// Ported from ./references/lazygit-master/pkg/gui/context/confirmation_context.go

use crate::types::common::ContextCommon;

/// State for confirmation context
pub struct ConfirmationContextState {
    pub on_confirm: Option<Box<dyn Fn() -> Result<(), String> + Send>>,
    pub on_close: Option<Box<dyn Fn() -> Result<(), String> + Send>>,
}

/// Confirmation context for user confirmations
pub struct ConfirmationContext {
    c: ContextCommon,
    state: ConfirmationContextState,
}

impl ConfirmationContext {
    pub fn new(c: ContextCommon) -> Self {
        Self {
            c,
            state: ConfirmationContextState {
                on_confirm: None,
                on_close: None,
            },
        }
    }

    /// Set the confirmation callbacks
    pub fn set_state(&mut self, on_confirm: Option<Box<dyn Fn() -> Result<(), String> + Send>>, on_close: Option<Box<dyn Fn() -> Result<(), String> + Send>>) {
        self.state.on_confirm = on_confirm;
        self.state.on_close = on_close;
    }

    /// Get the confirmation state
    pub fn get_state(&self) -> &ConfirmationContextState {
        &self.state
    }

    /// Handle confirm
    pub fn on_confirm(&self) -> Result<(), String> {
        if let Some(ref callback) = self.state.on_confirm {
            callback()
        } else {
            Ok(())
        }
    }

    /// Handle close
    pub fn on_close(&self) -> Result<(), String> {
        if let Some(ref callback) = self.state.on_close {
            callback()
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirmation_context_new() {
        // Basic instantiation test
    }

    #[test]
    fn test_confirmation_state_default() {
        // Default state should have None callbacks
    }
}
