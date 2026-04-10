// Ported from ./references/lazygit-master/pkg/theme/theme.go

use ratatui::style::Color;

use super::gocui::get_gocui_style;
use super::style::get_text_style;
use super::text_style::TextStyle;

/// ColorScheme represents the terminal color scheme (dark or light)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorScheme {
    Dark,
    Light,
}

impl Default for ColorScheme {
    fn default() -> Self {
        ColorScheme::Dark
    }
}

/// Cached terminal color scheme (set once at startup)
static TERMINAL_COLOR_SCHEME: std::sync::OnceLock<ColorScheme> = std::sync::OnceLock::new();

fn detect_terminal_color_scheme() -> ColorScheme {
    // COLORFGBG format: "foreground;background" (e.g., "0;15" = dark)
    if let Ok(colorfgbg) = std::env::var("COLORFGBG") {
        if let Some(parts) = colorfgbg.split(';').collect::<Vec<_>>().first() {
            if let Ok(bg_val) = parts.parse::<u8>() {
                if bg_val > 7 {
                    return ColorScheme::Dark;
                } else {
                    return ColorScheme::Light;
                }
            }
        }
    }
    ColorScheme::Dark
}

/// terminal_color_scheme returns the detected terminal color scheme.
/// Detection happens once at startup and is cached.
pub fn terminal_color_scheme() -> ColorScheme {
    *TERMINAL_COLOR_SCHEME.get_or_init(detect_terminal_color_scheme)
}

/// ColorPalette represents a set of colors for a specific color scheme
#[derive(Clone, Debug)]
pub struct ColorPalette {
    pub active_border: Vec<&'static str>,
    pub inactive_border: Vec<&'static str>,
    pub searching_active_border: Vec<&'static str>,
    pub selected_line_bg: Vec<&'static str>,
    pub inactive_view_selected_line_bg: Vec<&'static str>,
    pub options: Vec<&'static str>,
    pub cherry_picked_commit_bg: Vec<&'static str>,
    pub cherry_picked_commit_fg: Vec<&'static str>,
    pub marked_base_commit_bg: Vec<&'static str>,
    pub marked_base_commit_fg: Vec<&'static str>,
    pub options_fg: Vec<&'static str>,
    pub unstaged_changes: Vec<&'static str>,
}

fn dark_palette() -> ColorPalette {
    ColorPalette {
        active_border: vec!["green", "bold"],
        inactive_border: vec!["default"],
        searching_active_border: vec!["cyan", "bold"],
        selected_line_bg: vec!["blue"],
        inactive_view_selected_line_bg: vec!["bold"],
        options: vec!["green"],
        cherry_picked_commit_bg: vec!["magenta"],
        cherry_picked_commit_fg: vec!["default"],
        marked_base_commit_bg: vec!["cyan"],
        marked_base_commit_fg: vec!["default"],
        options_fg: vec!["green"],
        unstaged_changes: vec!["red"],
    }
}

fn light_palette() -> ColorPalette {
    ColorPalette {
        active_border: vec!["blue", "bold"],
        inactive_border: vec!["black"],
        searching_active_border: vec!["magenta", "bold"],
        selected_line_bg: vec!["cyan"],
        inactive_view_selected_line_bg: vec!["underline"],
        options: vec!["blue"],
        cherry_picked_commit_bg: vec!["red"],
        cherry_picked_commit_fg: vec!["default"],
        marked_base_commit_bg: vec!["green"],
        marked_base_commit_fg: vec!["default"],
        options_fg: vec!["blue"],
        unstaged_changes: vec!["red"],
    }
}

fn get_palette(scheme: ColorScheme) -> ColorPalette {
    match scheme {
        ColorScheme::Dark => dark_palette(),
        ColorScheme::Light => light_palette(),
    }
}

fn get_theme_palette() -> ColorPalette {
    get_palette(get_active_color_scheme())
}

pub fn get_active_color_scheme() -> ColorScheme {
    ColorScheme::Dark
}

pub fn default_text_color() -> TextStyle {
    TextStyle::new()
}

pub fn gocui_default_text_color() -> Color {
    Color::Reset
}

/// ActiveBorderColor is the border color of the active frame
pub fn active_border_color(config: &[&str]) -> Color {
    if config.is_empty() {
        return get_gocui_style(&get_theme_palette().active_border);
    }
    get_gocui_style(config)
}

/// InactiveBorderColor is the border color of the inactive frames
pub fn inactive_border_color(config: &[&str]) -> Color {
    if config.is_empty() {
        return get_gocui_style(&get_theme_palette().inactive_border);
    }
    get_gocui_style(config)
}

/// SearchingActiveBorderColor is the border color of the active frame when searching/filtering
pub fn searching_active_border_color(config: &[&str]) -> Color {
    if config.is_empty() {
        return get_gocui_style(&get_theme_palette().searching_active_border);
    }
    get_gocui_style(config)
}

/// GocuiSelectedLineBgColor is the background color for the selected line in gocui
pub fn gocui_selected_line_bg_color(config: &[&str]) -> Color {
    get_gocui_style(config)
}

/// GocuiInactiveViewSelectedLineBgColor is the background color for the selected line when view doesn't have focus
pub fn gocui_inactive_view_selected_line_bg_color(config: &[&str]) -> Color {
    get_gocui_style(config)
}

/// OptionsColor returns the color for options text
pub fn options_color(config: &[&str]) -> Color {
    if config.is_empty() {
        return get_gocui_style(&get_theme_palette().options);
    }
    get_gocui_style(config)
}

/// SelectedLineBgColor returns the background color for selected lines
pub fn selected_line_bg_color(config: &[&str]) -> TextStyle {
    if config.is_empty() {
        return get_text_style(&get_theme_palette().selected_line_bg, true);
    }
    get_text_style(config, true)
}

/// InactiveViewSelectedLineBgColor returns the background color for selected lines in inactive view
pub fn inactive_view_selected_line_bg_color(config: &[&str]) -> TextStyle {
    if config.is_empty() {
        return get_text_style(&get_theme_palette().inactive_view_selected_line_bg, true);
    }
    get_text_style(config, true)
}

/// CherryPickedCommitTextStyle returns the text style for cherry-picked commits
pub fn cherry_picked_commit_text_style(bg_config: &[&str], fg_config: &[&str]) -> TextStyle {
    let palette = get_theme_palette();
    let bg = if bg_config.is_empty() {
        &palette.cherry_picked_commit_bg
    } else {
        bg_config
    };
    let fg = if fg_config.is_empty() {
        &palette.cherry_picked_commit_fg
    } else {
        fg_config
    };
    let bg_style = get_text_style(bg, true);
    let fg_style = get_text_style(fg, false);
    bg_style.merge_style(fg_style)
}

/// MarkedBaseCommitTextStyle returns the text style for marked base commit
pub fn marked_base_commit_text_style(bg_config: &[&str], fg_config: &[&str]) -> TextStyle {
    let palette = get_theme_palette();
    let bg = if bg_config.is_empty() {
        &palette.marked_base_commit_bg
    } else {
        bg_config
    };
    let fg = if fg_config.is_empty() {
        &palette.marked_base_commit_fg
    } else {
        fg_config
    };
    let bg_style = get_text_style(bg, true);
    let fg_style = get_text_style(fg, false);
    bg_style.merge_style(fg_style)
}

/// OptionsFgColor returns the foreground color for options
pub fn options_fg_color(config: &[&str]) -> TextStyle {
    if config.is_empty() {
        return get_text_style(&get_theme_palette().options_fg, false);
    }
    get_text_style(config, false)
}

/// DiffTerminalColor returns the color for diff terminal
pub fn diff_terminal_color() -> TextStyle {
    TextStyle::new().set_fg(super::color::AppColor::new_basic(Color::Magenta))
}

/// UnstagedChangesColor returns the color for unstaged changes
pub fn unstaged_changes_color(config: &[&str]) -> TextStyle {
    if config.is_empty() {
        return get_text_style(&get_theme_palette().unstaged_changes, false);
    }
    get_text_style(config, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_colorfgbg_parsing_high_first_value() {
        assert_eq!(parse_colorfgbg_for_test("8;0"), Some(ColorScheme::Dark));
        assert_eq!(parse_colorfgbg_for_test("15;7"), Some(ColorScheme::Dark));
    }

    #[test]
    fn test_colorfgbg_parsing_low_first_value() {
        assert_eq!(parse_colorfgbg_for_test("0;15"), Some(ColorScheme::Light));
        assert_eq!(parse_colorfgbg_for_test("7;15"), Some(ColorScheme::Light));
    }

    #[test]
    fn test_colorfgbg_boundary_values() {
        assert_eq!(parse_colorfgbg_for_test("7;15"), Some(ColorScheme::Light));
        assert_eq!(parse_colorfgbg_for_test("8;15"), Some(ColorScheme::Dark));
    }

    #[test]
    fn test_colorfgbg_invalid_input() {
        assert_eq!(parse_colorfgbg_for_test("invalid"), None);
        assert_eq!(parse_colorfgbg_for_test(""), None);
        assert_eq!(parse_colorfgbg_for_test(";"), None);
    }

    #[test]
    fn test_colorfgbg_single_value() {
        assert_eq!(parse_colorfgbg_for_test("15"), Some(ColorScheme::Dark));
        assert_eq!(parse_colorfgbg_for_test("0"), Some(ColorScheme::Light));
    }

    #[test]
    fn test_color_scheme_default() {
        assert_eq!(ColorScheme::default(), ColorScheme::Dark);
    }

    #[test]
    fn test_dark_palette_colors() {
        let palette = dark_palette();
        assert_eq!(palette.active_border, vec!["green", "bold"]);
        assert_eq!(palette.inactive_border, vec!["default"]);
        assert_eq!(palette.selected_line_bg, vec!["blue"]);
        assert_eq!(palette.options, vec!["green"]);
        assert_eq!(palette.unstaged_changes, vec!["red"]);
    }

    #[test]
    fn test_light_palette_colors() {
        let palette = light_palette();
        assert_eq!(palette.active_border, vec!["blue", "bold"]);
        assert_eq!(palette.inactive_border, vec!["black"]);
        assert_eq!(palette.selected_line_bg, vec!["cyan"]);
        assert_eq!(palette.options, vec!["blue"]);
        assert_eq!(palette.unstaged_changes, vec!["red"]);
    }
}

fn parse_colorfgbg_for_test(colorfgbg: &str) -> Option<ColorScheme> {
    let parts: Vec<&str> = colorfgbg.split(';').collect();
    let value = parts.first()?;
    let val = value.parse::<u8>().ok()?;
    Some(if val > 7 {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    })
}
