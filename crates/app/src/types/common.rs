// Ported from ./references/lazygit-master/pkg/gui/types/common.go

pub struct HelperCommon;

pub struct ContextCommon;

pub trait IGuiCommon {
    fn log_action(&self, action: &str);
    fn log_command(&self, cmd: &str, is_cmd_line: bool);
    fn refresh(&self, opts: RefreshOptions);
    fn post_refresh_update(&self, ctx: Context);
    fn set_view_content(&self, view_name: &str, content: &str);
    fn reset_view_origin(&self, view_name: &str);
    fn render(&self);
    fn render_to_main_views(&self, opts: RefreshMainOpts);
    fn suspend(&self) -> Result<(), String>;
    fn resume(&self) -> Result<(), String>;
}

pub trait IModeMgr {
    fn is_any_mode_active(&self) -> bool;
}

pub trait IPopupHandler {
    fn error_handler(&self, err: String) -> Result<(), String>;
    fn alert(&self, title: &str, message: &str);
    fn confirm(&self, opts: ConfirmOpts);
    fn confirm_if(&self, condition: bool, opts: ConfirmOpts) -> Result<(), String>;
    fn prompt(&self, opts: PromptOpts);
    fn with_waiting_status(
        &self,
        message: &str,
        f: Box<dyn Fn() -> Result<(), String>>,
    ) -> Result<(), String>;
    fn with_waiting_status_sync(
        &self,
        message: &str,
        f: Box<dyn Fn() -> Result<(), String>>,
    ) -> Result<(), String>;
    fn menu(&self, opts: CreateMenuOptions) -> Result<(), String>;
    fn toast(&self, message: &str);
    fn error_toast(&self, message: &str);
    fn set_toast_func(&self, f: Box<dyn Fn(String)>);
    fn get_prompt_input(&self) -> String;
}

#[derive(Clone, Copy)]
pub enum ToastKind {
    Status,
    Error,
}

pub struct CreateMenuOptions {
    pub title: String,
    pub prompt: String,
    pub items: Vec<MenuItem>,
    pub hide_cancel: bool,
    pub column_alignment: Vec<i32>,
    pub allow_filtering_keybindings: bool,
    pub keep_conflicting_keybindings: bool,
}

pub struct CreatePopupPanelOpts {
    pub has_loader: bool,
    pub editable: bool,
    pub title: String,
    pub prompt: String,
    pub handle_confirm: Option<Box<dyn Fn() -> Result<(), String>>>,
    pub handle_confirm_prompt: Option<Box<dyn Fn(String) -> Result<(), String>>>,
    pub handle_close: Option<Box<dyn Fn() -> Result<(), String>>>,
    pub handle_delete_suggestion: Option<Box<dyn Fn(i32) -> Result<(), String>>>,
    pub find_suggestions_func: Option<Box<dyn Fn(String) -> Vec<Suggestion>>>,
    pub mask: bool,
    pub allow_edit_suggestion: bool,
    pub allow_empty_input: bool,
    pub preserve_whitespace: bool,
}

pub struct ConfirmOpts {
    pub title: String,
    pub prompt: String,
    pub handle_confirm: Option<Box<dyn Fn() -> Result<(), String>>>,
    pub handle_close: Option<Box<dyn Fn() -> Result<(), String>>>,
    pub find_suggestions_func: Option<Box<dyn Fn(String) -> Vec<Suggestion>>>,
    pub editable: bool,
    pub mask: bool,
}

pub struct PromptOpts {
    pub title: String,
    pub initial_content: String,
    pub find_suggestions_func: Option<Box<dyn Fn(String) -> Vec<Suggestion>>>,
    pub handle_confirm: Option<Box<dyn Fn(String) -> Result<(), String>>>,
    pub allow_edit_suggestion: bool,
    pub allow_empty_input: bool,
    pub preserve_whitespace: bool,
    pub handle_close: Option<Box<dyn Fn() -> Result<(), String>>>,
    pub handle_delete_suggestion: Option<Box<dyn Fn(i32) -> Result<(), String>>>,
    pub mask: bool,
}

pub struct MenuSection {
    pub title: String,
    pub column: i32,
}

pub struct DisabledReason {
    pub text: String,
    pub show_error_in_panel: bool,
    pub allow_further_dispatching: bool,
}

#[derive(Clone, Copy)]
pub enum MenuWidget {
    None,
    RadioButtonSelected,
    RadioButtonUnselected,
    CheckboxSelected,
    CheckboxUnselected,
}

pub struct MenuItem {
    pub label: String,
    pub label_columns: Vec<String>,
    pub on_press: Option<Box<dyn Fn() -> Result<(), String>>>,
    pub opens_menu: bool,
    pub key: Key,
    pub widget: MenuWidget,
    pub tooltip: String,
    pub disabled_reason: Option<DisabledReason>,
    pub section: Option<MenuSection>,
}

#[derive(Clone, Copy)]
pub enum ItemOperation {
    None,
    Pushing,
    Pulling,
    FastForwarding,
    Deleting,
    Fetching,
    CheckingOut,
}

pub trait HasUrn {
    fn urn(&self) -> String;
}

#[derive(Clone, Copy)]
pub enum StartupStage {
    Initial,
    Complete,
}

#[derive(Clone, Copy)]
pub enum ScreenMode {
    Normal,
    Half,
    Full,
}

pub struct Mutexes;

pub struct Model;

pub struct RefreshOptions;

pub struct RefreshMainOpts;

pub struct Context;

pub struct Key;

pub struct Suggestion;
