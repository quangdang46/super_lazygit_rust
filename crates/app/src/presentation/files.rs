use crate::style::basic_styles::{
    fg_black, fg_blue, fg_cyan, fg_green, fg_magenta, fg_red, fg_yellow,
};
use crate::style::text_style::TextStyle;
use crate::style::theme::default_text_color;
use crate::utils::escape_special_chars;

use super_lazygit_git::patch::PatchStatus;

pub const EXPANDED_ARROW: &str = "\u{25bc}";
pub const COLLAPSED_ARROW: &str = "\u{25b6}";

pub fn format_file_status(file_short_status: &str, rest_style: TextStyle) -> String {
    if file_short_status.len() < 2 {
        return file_short_status.to_string();
    }

    let first_char = &file_short_status[0..1];
    let first_char_style = match first_char {
        "?" => fg_red(),
        " " => rest_style,
        _ => fg_green(),
    };

    let second_char = &file_short_status[1..2];
    let second_char_style = if second_char == " " {
        rest_style
    } else {
        fg_red()
    };

    first_char_style.sprint(first_char) + &second_char_style.sprint(second_char)
}

pub fn format_line_changes(lines_added: u32, lines_deleted: u32) -> String {
    let mut output = String::new();

    if lines_added != 0 {
        output.push_str(&fg_green().sprint(&format!("+{lines_added}")));
    }

    if lines_deleted != 0 {
        if !output.is_empty() {
            output.push(' ');
        }
        output.push_str(&fg_red().sprint(&format!("-{lines_deleted}")));
    }

    output
}

pub fn get_color_for_change_status(change_status: &str) -> TextStyle {
    match change_status {
        "A" => fg_green(),
        "M" | "R" => fg_yellow(),
        "D" => fg_red(),
        "C" => fg_cyan(),
        "T" => fg_magenta(),
        _ => default_text_color(),
    }
}

pub fn file_name_at_depth(internal_path: &str, depth: usize) -> String {
    let split: Vec<&str> = internal_path.split('/').collect();
    let mut adj_depth = depth;

    if adj_depth == 0 && split.first() == Some(&".") {
        if split.len() == 1 {
            return "/".to_string();
        }
        adj_depth = 1;
    }

    if adj_depth >= split.len() {
        return split.last().unwrap_or(&"").to_string();
    }

    split[adj_depth..].join("/")
}

pub fn commit_file_name_at_depth(internal_path: &str, depth: usize) -> String {
    file_name_at_depth(internal_path, depth)
}

pub fn get_file_color(has_staged_changes: bool, has_unstaged_changes: bool) -> TextStyle {
    if has_staged_changes && !has_unstaged_changes {
        fg_green()
    } else if has_staged_changes {
        fg_yellow()
    } else {
        default_text_color()
    }
}

pub fn get_patch_status_color(status: PatchStatus) -> TextStyle {
    match status {
        PatchStatus::Whole => fg_green(),
        PatchStatus::Part => fg_yellow(),
        PatchStatus::Unselected => default_text_color(),
    }
}

pub fn get_patch_status_symbol(
    status: PatchStatus,
    change_status: Option<&str>,
) -> (String, TextStyle) {
    match status {
        PatchStatus::Whole => ("\u{25cf}".to_string(), fg_green()),
        PatchStatus::Part => ("\u{25d0}".to_string(), fg_yellow()),
        PatchStatus::Unselected => {
            let cs = change_status.unwrap_or("");
            (cs.to_string(), get_color_for_change_status(cs))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expanded_arrow() {
        assert_eq!(EXPANDED_ARROW, "\u{25bc}");
    }

    #[test]
    fn test_collapsed_arrow() {
        assert_eq!(COLLAPSED_ARROW, "\u{25b6}");
    }

    #[test]
    fn test_format_file_status_staged() {
        let result = format_file_status("M ", default_text_color());
        assert!(result.contains("M"));
    }

    #[test]
    fn test_format_file_status_untracked() {
        let result = format_file_status("??", default_text_color());
        assert!(result.contains("?"));
    }

    #[test]
    fn test_format_file_status_short_input() {
        let result = format_file_status("M", default_text_color());
        assert_eq!(result, "M");
    }

    #[test]
    fn test_format_line_changes_both_nonzero() {
        let result = format_line_changes(5, 3);
        assert!(result.contains("+5"));
        assert!(result.contains("-3"));
    }

    #[test]
    fn test_format_line_changes_only_added() {
        let result = format_line_changes(10, 0);
        assert!(result.contains("+10"));
    }

    #[test]
    fn test_format_line_changes_only_deleted() {
        let result = format_line_changes(0, 7);
        assert!(result.contains("-7"));
    }

    #[test]
    fn test_format_line_changes_both_zero() {
        let result = format_line_changes(0, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_color_for_change_status_a() {
        let style = get_color_for_change_status("A");
        assert_eq!(style, fg_green());
    }

    #[test]
    fn test_get_color_for_change_status_m() {
        let style = get_color_for_change_status("M");
        assert_eq!(style, fg_yellow());
    }

    #[test]
    fn test_get_color_for_change_status_r() {
        let style = get_color_for_change_status("R");
        assert_eq!(style, fg_yellow());
    }

    #[test]
    fn test_get_color_for_change_status_d() {
        let style = get_color_for_change_status("D");
        assert_eq!(style, fg_red());
    }

    #[test]
    fn test_get_color_for_change_status_c() {
        let style = get_color_for_change_status("C");
        assert_eq!(style, fg_cyan());
    }

    #[test]
    fn test_get_color_for_change_status_t() {
        let style = get_color_for_change_status("T");
        assert_eq!(style, fg_magenta());
    }

    #[test]
    fn test_get_color_for_change_status_unknown() {
        let style = get_color_for_change_status("X");
        assert_eq!(style, default_text_color());
    }

    #[test]
    fn test_file_name_at_depth_basic() {
        assert_eq!(file_name_at_depth("src/gui/main.go", 0), "src/gui/main.go");
        assert_eq!(file_name_at_depth("src/gui/main.go", 1), "gui/main.go");
        assert_eq!(file_name_at_depth("src/gui/main.go", 2), "main.go");
    }

    #[test]
    fn test_file_name_at_depth_dot_prefix() {
        assert_eq!(file_name_at_depth("./src/main.go", 0), "src/main.go");
        assert_eq!(file_name_at_depth(".", 0), "/");
    }

    #[test]
    fn test_file_name_at_depth_beyond_end() {
        assert_eq!(file_name_at_depth("src/main.go", 10), "main.go");
    }

    #[test]
    fn test_get_patch_status_color_whole() {
        assert_eq!(get_patch_status_color(PatchStatus::Whole), fg_green());
    }

    #[test]
    fn test_get_patch_status_color_part() {
        assert_eq!(get_patch_status_color(PatchStatus::Part), fg_yellow());
    }

    #[test]
    fn test_get_patch_status_color_unselected() {
        assert_eq!(
            get_patch_status_color(PatchStatus::Unselected),
            default_text_color()
        );
    }

    #[test]
    fn test_get_patch_status_symbol_whole() {
        let (symbol, _) = get_patch_status_symbol(PatchStatus::Whole, None);
        assert_eq!(symbol, "\u{25cf}");
    }

    #[test]
    fn test_get_patch_status_symbol_part() {
        let (symbol, _) = get_patch_status_symbol(PatchStatus::Part, None);
        assert_eq!(symbol, "\u{25d0}");
    }

    #[test]
    fn test_get_patch_status_symbol_unselected_with_change() {
        let (symbol, style) = get_patch_status_symbol(PatchStatus::Unselected, Some("M"));
        assert_eq!(symbol, "M");
        assert_eq!(style, fg_yellow());
    }
}
