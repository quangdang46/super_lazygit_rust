// Ported from ./references/lazygit-master/pkg/gui/keybindings.go

pub struct Gui;

impl Gui {
    pub fn no_popup_panel(
        &self,
        _f: fn() -> Result<(), String>,
    ) -> Box<dyn Fn() -> Result<(), String>> {
        Box::new(|| Ok(()))
    }

    pub fn outside_filter_mode(
        &self,
        _f: fn() -> Result<(), String>,
    ) -> Box<dyn Fn() -> Result<(), String>> {
        Box::new(|| Ok(()))
    }

    pub fn validate_not_in_filter_mode(&self) -> bool {
        false
    }

    pub fn get_cheatsheet_keybindings(&self) -> Vec<Binding> {
        Vec::new()
    }

    pub fn keybinding_opts(&self) -> KeybindingsOpts {
        KeybindingsOpts
    }

    pub fn get_initial_keybindings(&self) -> (Vec<Binding>, Vec<ViewMouseBinding>) {
        (Vec::new(), Vec::new())
    }

    pub fn get_initial_keybindings_with_custom_commands(
        &self,
    ) -> (Vec<Binding>, Vec<ViewMouseBinding>) {
        (Vec::new(), Vec::new())
    }

    pub fn reset_keybindings(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn wrapped_handler(
        &self,
        _f: fn() -> Result<(), String>,
    ) -> Box<dyn Fn() -> Result<(), String>> {
        Box::new(|| Ok(()))
    }

    pub fn set_keybinding(&self, _binding: &Binding) -> Result<(), String> {
        Ok(())
    }

    pub fn set_mouse_keybinding(&self, _binding: &ViewMouseBinding) -> Result<(), String> {
        Ok(())
    }

    pub fn call_keybinding_handler(&self, _binding: &Binding) -> Result<(), String> {
        Ok(())
    }
}

pub struct Binding;
pub struct ViewMouseBinding;
pub struct KeybindingsOpts;
