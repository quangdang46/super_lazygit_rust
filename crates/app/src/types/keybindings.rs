// Ported from ./references/lazygit-master/pkg/gui/types/keybindings.go

pub type Key = Box<dyn std::any::Any>;

pub struct Binding {
    pub view_name: String,
    pub handler: Option<Box<dyn Fn() -> Result<(), String>>>,
    pub key: Key,
    pub modifier: u32,
    pub description: String,
    pub description_func: Option<Box<dyn Fn() -> String>>,
    pub short_description: String,
    pub short_description_func: Option<Box<dyn Fn() -> String>>,
    pub alternative: String,
    pub tag: String,
    pub opens_menu: bool,
    pub display_on_screen: bool,
    pub display_style: Option<TextStyle>,
    pub tooltip: String,
    pub get_disabled_reason: Option<Box<dyn Fn() -> Option<DisabledReason>>>,
}

impl Binding {
    pub fn is_disabled(&self) -> bool {
        if let Some(ref f) = self.get_disabled_reason {
            return f().is_some();
        }
        false
    }

    pub fn get_description(&self) -> String {
        if let Some(ref f) = self.description_func {
            return f();
        }
        self.description.clone()
    }

    pub fn get_short_description(&self) -> String {
        if let Some(ref f) = self.short_description_func {
            return f();
        }
        if !self.short_description.is_empty() {
            return self.short_description.clone();
        }
        self.get_description()
    }
}

pub type Guard =
    Box<dyn Fn(Box<dyn Fn() -> Result<(), String>>) -> Box<dyn Fn() -> Result<(), String>>>;

pub struct KeybindingGuards {
    pub outside_filter_mode: Guard,
    pub no_popup_panel: Guard,
}

pub struct ErrKeybindingNotHandled {
    pub disabled_reason: Option<DisabledReason>,
}

impl std::fmt::Display for ErrKeybindingNotHandled {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(ref reason) = self.disabled_reason {
            write!(f, "{}", reason.text)
        } else {
            write!(f, "keybinding not handled")
        }
    }
}

pub struct TextStyle;
