// Ported from ./references/lazygit-master/pkg/gui/popup/popup_handler.go

pub struct CreatePopupPanelOpts {
    pub title: String,
    pub prompt: String,
    pub handle_confirm: Option<Box<dyn Fn()>>,
    pub handle_close: Option<Box<dyn Fn()>>,
    pub editable: bool,
    pub handle_confirm_prompt: Option<Box<dyn Fn(String)>>,
    pub handle_delete_suggestion: Option<Box<dyn Fn(String)>>,
    pub find_suggestions_func: Option<Box<dyn Fn(String) -> Vec<Suggestion>>>,
    pub allow_edit_suggestion: bool,
    pub allow_empty_input: bool,
    pub preserve_whitespace: bool,
    pub mask: bool,
}

pub struct CreateMenuOptions {
    pub title: String,
}

pub struct ToastKind;

pub struct Suggestion;

pub struct ConfirmOpts {
    pub title: String,
    pub prompt: String,
    pub handle_confirm: Option<Box<dyn Fn()>>,
    pub handle_close: Option<Box<dyn Fn()>>,
}

pub struct PromptOpts {
    pub title: String,
    pub initial_content: String,
    pub handle_confirm: Option<Box<dyn Fn(String)>>,
    pub handle_close: Option<Box<dyn Fn()>>,
    pub handle_delete_suggestion: Option<Box<dyn Fn(String)>>,
    pub find_suggestions_func: Option<Box<dyn Fn(String) -> Vec<Suggestion>>>,
    pub allow_edit_suggestion: bool,
    pub allow_empty_input: bool,
    pub preserve_whitespace: bool,
    pub mask: bool,
}

pub type PopupFn = Box<dyn Fn(String, CreatePopupPanelOpts)>;
pub type ErrorFn = Box<dyn Fn() -> Result<(), String>>;
pub type ContextFn = Box<dyn Fn()>;
pub type MenuFn = Box<dyn Fn(CreateMenuOptions) -> Result<(), String>>;
pub type WaitingFn = Box<dyn Fn(String, Box<dyn Fn()>)>;
pub type WaitingSyncFn =
    Box<dyn Fn(String, Box<dyn Fn() -> Result<(), String>>) -> Result<(), String>>;
pub type ToastFn = Box<dyn Fn(String, ToastKind)>;
pub type PromptInputFn = Box<dyn Fn() -> String>;
pub type DemoFn = Box<dyn Fn() -> bool>;

pub struct PopupHandler {
    pub create_popup_panel_fn: PopupFn,
    pub on_error_fn: ErrorFn,
    pub pop_context_fn: ContextFn,
    pub current_context_fn: Box<dyn Fn()>,
    pub create_menu_fn: MenuFn,
    pub with_waiting_status_fn: WaitingFn,
    pub with_waiting_status_sync_fn: WaitingSyncFn,
    pub toast_fn: ToastFn,
    pub get_prompt_input_fn: PromptInputFn,
    pub in_demo: DemoFn,
}

impl PopupHandler {
    pub fn menu(&self, opts: CreateMenuOptions) -> Result<(), String> {
        (self.create_menu_fn)(opts)
    }

    pub fn toast(&self, message: String) {
        (self.toast_fn)(message, ToastKind)
    }

    pub fn error_toast(&self, message: String) {
        (self.toast_fn)(message, ToastKind)
    }

    pub fn set_toast_func(&mut self, f: ToastFn) {
        self.toast_fn = f;
    }

    pub fn with_waiting_status(&self, message: String, f: Box<dyn Fn()>) -> Result<(), String> {
        (self.with_waiting_status_fn)(message, f);
        Ok(())
    }

    pub fn with_waiting_status_sync(
        &self,
        message: String,
        f: Box<dyn Fn() -> Result<(), String>>,
    ) -> Result<(), String> {
        (self.with_waiting_status_sync_fn)(message, f)
    }

    pub fn error_handler(&self, err: String) -> Result<(), String> {
        let colored_message = format!("[RED]{}[/RED]", err.trim());
        (self.on_error_fn)()?;
        self.alert("Error".to_string(), colored_message);
        Ok(())
    }

    pub fn alert(&self, title: String, message: String) {
        self.confirm(ConfirmOpts {
            title,
            prompt: message,
            handle_confirm: None,
            handle_close: None,
        });
    }

    pub fn confirm(&self, opts: ConfirmOpts) {
        (self.create_popup_panel_fn)(
            "confirm".to_string(),
            CreatePopupPanelOpts {
                title: opts.title,
                prompt: opts.prompt,
                handle_confirm: opts.handle_confirm,
                handle_close: opts.handle_close,
                editable: false,
                handle_confirm_prompt: None,
                handle_delete_suggestion: None,
                find_suggestions_func: None,
                allow_edit_suggestion: false,
                allow_empty_input: false,
                preserve_whitespace: false,
                mask: false,
            },
        );
    }

    pub fn confirm_if(&self, condition: bool, opts: ConfirmOpts) -> Result<(), String> {
        if condition {
            (self.create_popup_panel_fn)(
                "confirm".to_string(),
                CreatePopupPanelOpts {
                    title: opts.title,
                    prompt: opts.prompt,
                    handle_confirm: opts.handle_confirm,
                    handle_close: opts.handle_close,
                    editable: false,
                    handle_confirm_prompt: None,
                    handle_delete_suggestion: None,
                    find_suggestions_func: None,
                    allow_edit_suggestion: false,
                    allow_empty_input: false,
                    preserve_whitespace: false,
                    mask: false,
                },
            );
            Ok(())
        } else if let Some(handler) = opts.handle_confirm {
            handler();
            Ok(())
        } else {
            Ok(())
        }
    }

    pub fn prompt(&self, opts: PromptOpts) {
        (self.create_popup_panel_fn)(
            "prompt".to_string(),
            CreatePopupPanelOpts {
                title: opts.title,
                prompt: opts.initial_content,
                handle_confirm: None,
                handle_close: opts.handle_close,
                editable: true,
                handle_confirm_prompt: opts.handle_confirm,
                handle_delete_suggestion: opts.handle_delete_suggestion,
                find_suggestions_func: opts.find_suggestions_func,
                allow_edit_suggestion: opts.allow_edit_suggestion,
                allow_empty_input: opts.allow_empty_input,
                preserve_whitespace: opts.preserve_whitespace,
                mask: opts.mask,
            },
        );
    }

    pub fn get_prompt_input(&self) -> String {
        (self.get_prompt_input_fn)()
    }
}
