// Ported from ./references/lazygit-master/pkg/gui/style/text_style.go

use ratatui::style::Style;

use super::color::AppColor;
use super::decoration::Decoration;

#[derive(Clone, Copy)]
pub struct TextStyle {
    fg: Option<AppColor>,
    bg: Option<AppColor>,
    decoration: Decoration,
}

impl TextStyle {
    pub fn new() -> Self {
        Self {
            fg: None,
            bg: None,
            decoration: Decoration::new(),
        }
    }

    pub fn set_fg(mut self, color: AppColor) -> Self {
        self.fg = Some(color);
        self
    }

    pub fn set_bg(mut self, color: AppColor) -> Self {
        self.bg = Some(color);
        self
    }

    pub fn set_bold(mut self) -> Self {
        self.decoration.set_bold();
        self
    }

    pub fn set_underline(mut self) -> Self {
        self.decoration.set_underline();
        self
    }

    pub fn set_reverse(mut self) -> Self {
        self.decoration.set_reverse();
        self
    }

    pub fn set_strikethrough(mut self) -> Self {
        self.decoration.set_strikethrough();
        self
    }

    pub fn merge_style(&self, other: TextStyle) -> TextStyle {
        let mut result = *self;
        result.decoration = self.decoration.merge(&other.decoration);
        if other.fg.is_some() {
            result.fg = other.fg;
        }
        if other.bg.is_some() {
            result.bg = other.bg;
        }
        result
    }

    pub fn to_ratatui_style(&self) -> Style {
        let mut style = Style::default();

        if let Some(fg_color) = &self.fg {
            style = style.fg(fg_color.to_ratatui_color(false));
        }

        if let Some(bg_color) = &self.bg {
            style = style.bg(bg_color.to_ratatui_color(true));
        }

        for modifier in self.decoration.to_modifiers() {
            style = style.add_modifier(modifier);
        }

        style
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::new()
    }
}

impl TextStyle {
    pub fn sprint(&self, text: &str) -> String {
        text.to_string()
    }

    pub fn sprintf(&self, format: &str, args: &[&str]) -> String {
        format.replace("{}", &args.join(""))
    }
}
