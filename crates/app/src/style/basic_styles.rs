// Ported from ./references/lazygit-master/pkg/gui/style/basic_styles.go

use ratatui::style::Color;

use super::color::AppColor;
use super::text_style::TextStyle;

pub fn from_basic_fg(color: Color) -> TextStyle {
    TextStyle::new().set_fg(AppColor::new_basic(color))
}

pub fn from_basic_bg(color: Color) -> TextStyle {
    TextStyle::new().set_bg(AppColor::new_basic(color))
}

pub fn fg_white() -> TextStyle {
    from_basic_fg(Color::White)
}
pub fn fg_light_white() -> TextStyle {
    from_basic_fg(Color::White)
}
pub fn fg_black() -> TextStyle {
    from_basic_fg(Color::Black)
}
pub fn fg_black_lighter() -> TextStyle {
    from_basic_fg(Color::DarkGray)
}
pub fn fg_cyan() -> TextStyle {
    from_basic_fg(Color::Cyan)
}
pub fn fg_red() -> TextStyle {
    from_basic_fg(Color::Red)
}
pub fn fg_green() -> TextStyle {
    from_basic_fg(Color::Green)
}
pub fn fg_blue() -> TextStyle {
    from_basic_fg(Color::Blue)
}
pub fn fg_yellow() -> TextStyle {
    from_basic_fg(Color::Yellow)
}
pub fn fg_magenta() -> TextStyle {
    from_basic_fg(Color::Magenta)
}
pub fn fg_default() -> TextStyle {
    from_basic_fg(Color::Reset)
}

pub fn bg_white() -> TextStyle {
    from_basic_bg(Color::White)
}
pub fn bg_black() -> TextStyle {
    from_basic_bg(Color::Black)
}
pub fn bg_red() -> TextStyle {
    from_basic_bg(Color::Red)
}
pub fn bg_green() -> TextStyle {
    from_basic_bg(Color::Green)
}
pub fn bg_yellow() -> TextStyle {
    from_basic_bg(Color::Yellow)
}
pub fn bg_blue() -> TextStyle {
    from_basic_bg(Color::Blue)
}
pub fn bg_magenta() -> TextStyle {
    from_basic_bg(Color::Magenta)
}
pub fn bg_cyan() -> TextStyle {
    from_basic_bg(Color::Cyan)
}
pub fn bg_default() -> TextStyle {
    from_basic_bg(Color::Reset)
}

pub fn nothing() -> TextStyle {
    TextStyle::new()
}

pub fn attr_underline() -> TextStyle {
    TextStyle::new().set_underline()
}
pub fn attr_bold() -> TextStyle {
    TextStyle::new().set_bold()
}

pub struct ColorMapEntry {
    pub foreground: TextStyle,
    pub background: TextStyle,
}

pub fn color_map() -> std::collections::HashMap<&'static str, ColorMapEntry> {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "default",
        ColorMapEntry {
            foreground: fg_default(),
            background: bg_default(),
        },
    );
    map.insert(
        "black",
        ColorMapEntry {
            foreground: fg_black(),
            background: bg_black(),
        },
    );
    map.insert(
        "red",
        ColorMapEntry {
            foreground: fg_red(),
            background: bg_red(),
        },
    );
    map.insert(
        "green",
        ColorMapEntry {
            foreground: fg_green(),
            background: bg_green(),
        },
    );
    map.insert(
        "yellow",
        ColorMapEntry {
            foreground: fg_yellow(),
            background: bg_yellow(),
        },
    );
    map.insert(
        "blue",
        ColorMapEntry {
            foreground: fg_blue(),
            background: bg_blue(),
        },
    );
    map.insert(
        "magenta",
        ColorMapEntry {
            foreground: fg_magenta(),
            background: bg_magenta(),
        },
    );
    map.insert(
        "cyan",
        ColorMapEntry {
            foreground: fg_cyan(),
            background: bg_cyan(),
        },
    );
    map.insert(
        "white",
        ColorMapEntry {
            foreground: fg_white(),
            background: bg_white(),
        },
    );
    map
}
