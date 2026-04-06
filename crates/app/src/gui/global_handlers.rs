// Ported from ./references/lazygit-master/pkg/gui/global_handlers.go

pub struct Gui;

const HORIZONTAL_SCROLL_FACTOR: i32 = 3;

impl Gui {
    pub fn scroll_up_view(&self, _view: &str) {}

    pub fn scroll_down_view(&self, _view: &str) {}

    pub fn scroll_up_main(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn scroll_down_main(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn main_view(&self) -> String {
        String::new()
    }

    pub fn secondary_view(&self) -> String {
        String::new()
    }

    pub fn scroll_up_secondary(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn scroll_down_secondary(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn scroll_up_confirmation_panel(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn scroll_down_confirmation_panel(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn page_up_confirmation_panel(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn page_down_confirmation_panel(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn go_to_confirmation_panel_top(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn go_to_confirmation_panel_bottom(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_copy_selected_side_context_item_to_clipboard(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_copy_selected_side_context_item_commit_hash_to_clipboard(
        &self,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_copy_selected_side_context_item_to_clipboard_with_truncation(
        &self,
        _max_width: i32,
    ) -> Result<(), String> {
        Ok(())
    }

    fn get_copy_selected_side_context_item_to_clipboard_disabled_reason(
        &self,
    ) -> Option<DisabledReason> {
        None
    }

    pub fn set_caption(&self, _caption: String) {}

    pub fn set_caption_prefix(&self, _prefix: String) {}
}

pub struct DisabledReason {
    pub text: String,
}
