// Ported from ./references/lazygit-master/pkg/integration/types/types.go

pub trait IntegrationTest {
    fn run(&self, gui_driver: &GuiDriver);
    fn setup_config(&self, config: &AppConfig);
    fn requires_headless(&self) -> bool;
    fn headless_dimensions(&self) -> (i32, i32);
    fn is_demo(&self) -> bool;
}

pub trait GuiDriver {
    fn press_key(&self, key: &str);
    fn click(&self, x: i32, y: i32);
    fn keys(&self) -> KeybindingConfig;
    fn current_context(&self) -> Context;
    fn context_for_view(&self, view_name: &str) -> Context;
    fn fail(&self, message: &str);
    fn log(&self, message: &str);
    fn log_ui(&self, message: &str);
    fn checked_out_ref(&self) -> Branch;
    fn main_view(&self) -> View;
    fn secondary_view(&self) -> View;
    fn view(&self, view_name: &str) -> View;
    fn set_caption(&self, caption: &str);
    fn set_caption_prefix(&self, prefix: &str);
    fn next_toast(&self) -> Option<String>;
    fn check_all_toasts_acknowledged(&self);
    fn headless(&self) -> bool;
}

pub struct AppConfig;
pub struct KeybindingConfig;
pub struct Context;
pub struct Branch;
pub struct View;
