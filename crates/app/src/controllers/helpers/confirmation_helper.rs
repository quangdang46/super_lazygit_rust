// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/confirmation_helper.go

pub struct Context;

pub struct ConfirmOpts {
    pub title: String,
    pub prompt: String,
}

pub struct CreatePopupPanelOpts {
    pub title: String,
    pub prompt: String,
    pub editable: bool,
}

pub struct MenuItem {
    pub label: String,
}

pub struct HelperCommon;

pub struct ConfirmationHelper {
    context: HelperCommon,
}

impl ConfirmationHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
        }
    }

    fn close_and_call_confirmation_function(
        &self,
        _cancel: fn(),
        _function: Box<dyn Fn() -> Result<(), String>>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn wrapped_confirmation_function(
        &self,
        cancel: fn(),
        function: fn() -> Result<(), String>,
    ) -> Box<dyn Fn() -> Result<(), String>> {
        Box::new(move || {
            cancel();
            function()
        })
    }

    fn wrapped_prompt_confirmation_function(
        &self,
        cancel: fn(),
        function: fn(String) -> Result<(), String>,
        get_response: fn() -> String,
        allow_empty_input: bool,
        _preserve_whitespace: bool,
    ) -> Box<dyn Fn() -> Result<(), String>> {
        Box::new(move || {
            let response = get_response();
            if response.is_empty() && !allow_empty_input {
                return Ok(());
            }
            cancel();
            function(response)
        })
    }

    pub fn deactivate_confirmation(&self) {}

    pub fn deactivate_prompt(&self) {}

    fn get_popup_panel_dimensions_for_content_height(
        &self,
        _content_width: i64,
        _content_height: i64,
        _parent_popup_context: &Context,
    ) -> (i64, i64, i64, i64) {
        (0, 0, 0, 0)
    }

    fn get_popup_panel_dimensions_aux(
        &self,
        _content_width: i64,
        _content_height: i64,
        _parent_popup_context: &Context,
    ) -> (i64, i64, i64, i64) {
        (0, 0, 0, 0)
    }

    fn get_popup_panel_width(&self, max_width: i64) -> i64 {
        max_width
    }

    fn prepare_confirmation_panel(&self, _opts: &ConfirmOpts) {}

    fn prepare_prompt_panel(&self, _opts: &ConfirmOpts) {}

    pub fn create_popup_panel(&self, _opts: &CreatePopupPanelOpts) {}

    fn set_confirmation_key_bindings(&self, _cancel: fn(), _opts: &CreatePopupPanelOpts) {}

    fn set_prompt_key_bindings(&self, _cancel: fn(), _opts: &CreatePopupPanelOpts) {}

    fn clear_confirmation_view_key_bindings(&self) {}

    fn clear_prompt_view_key_bindings(&self) {}

    fn get_selected_suggestion_value(&self) -> String {
        String::new()
    }

    pub fn resize_current_popup_panels(&self) {}

    fn resize_menu(&self, _parent_popup_context: &Context) {}

    fn layout_menu_prompt(&self, _content_width: i64) -> i64 {
        0
    }

    fn resize_confirmation_panel(&self, _parent_popup_context: &Context) {}

    fn resize_prompt_panel(&self, _parent_popup_context: &Context) {}

    pub fn resize_commit_message_panels(&self, _parent_popup_context: &Context) {}

    pub fn is_popup_panel(&self, _context: &Context) -> bool {
        false
    }

    pub fn is_popup_panel_focused(&self) -> bool {
        false
    }

    pub fn tooltip_for_menu_item(&self, _menu_item: &MenuItem) -> String {
        String::new()
    }
}

impl Default for ConfirmationHelper {
    fn default() -> Self {
        Self::new()
    }
}
