// Ported from ./references/lazygit-master/pkg/theme/theme.go

use ratatui::style::Color;

use super::basic_styles::color_map;
use super::gocui::{get_gocui_attribute, get_gocui_style};
use super::style::get_text_style;
use super::text_style::TextStyle;

/// DefaultTextColor is the default text color
pub fn default_text_color() -> TextStyle {
    TextStyle::new()
}

/// GocuiDefaultTextColor is the same as DefaultTextColor but uses raw gocui colors
pub fn gocui_default_text_color() -> Color {
    Color::Reset
}

/// ActiveBorderColor is the border color of the active frame
pub fn active_border_color(config: &[&str]) -> Color {
    get_gocui_style(config)
}

/// InactiveBorderColor is the border color of the inactive frames
pub fn inactive_border_color(config: &[&str]) -> Color {
    get_gocui_style(config)
}

/// SearchingActiveBorderColor is the border color of the active frame when searching/filtering
pub fn searching_active_border_color(config: &[&str]) -> Color {
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
    get_gocui_style(config)
}

/// SelectedLineBgColor returns the background color for selected lines
pub fn selected_line_bg_color(config: &[&str]) -> TextStyle {
    get_text_style(config, true)
}

/// InactiveViewSelectedLineBgColor returns the background color for selected lines in inactive view
pub fn inactive_view_selected_line_bg_color(config: &[&str]) -> TextStyle {
    get_text_style(config, true)
}

/// CherryPickedCommitTextStyle returns the text style for cherry-picked commits
pub fn cherry_picked_commit_text_style(bg_config: &[&str], fg_config: &[&str]) -> TextStyle {
    let bg_style = get_text_style(bg_config, true);
    let fg_style = get_text_style(fg_config, false);
    bg_style.merge_style(fg_style)
}

/// MarkedBaseCommitTextStyle returns the text style for marked base commit
pub fn marked_base_commit_text_style(bg_config: &[&str], fg_config: &[&str]) -> TextStyle {
    let bg_style = get_text_style(bg_config, true);
    let fg_style = get_text_style(fg_config, false);
    bg_style.merge_style(fg_style)
}

/// OptionsFgColor returns the foreground color for options
pub fn options_fg_color(config: &[&str]) -> TextStyle {
    get_text_style(config, false)
}

/// DiffTerminalColor returns the color for diff terminal
pub fn diff_terminal_color() -> TextStyle {
    TextStyle::new().set_fg(super::color::AppColor::new_basic(Color::Magenta))
}

/// UnstagedChangesColor returns the color for unstaged changes
pub fn unstaged_changes_color(config: &[&str]) -> TextStyle {
    get_text_style(config, false)
}
