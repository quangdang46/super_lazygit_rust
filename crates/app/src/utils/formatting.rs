use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct ColumnConfig {
    pub width: usize,
    pub alignment: Alignment,
}

pub fn string_width(s: &str) -> usize {
    for byte in s.bytes() {
        if byte > 127 {
            return UnicodeWidthStr::width(s);
        }
    }
    s.len()
}

pub fn truncate_with_ellipsis(text: &str, limit: usize) -> String {
    if string_width(text) <= limit {
        return text.to_string();
    }
    if limit == 0 {
        return String::new();
    }
    if limit == 1 {
        return "…".to_string();
    }

    let mut truncated = String::new();
    let mut current_width = 0;

    for grapheme in unicode_segmentation::UnicodeSegmentation::graphemes(text, true) {
        let grapheme_width = string_width(grapheme);
        if current_width + grapheme_width >= limit {
            break;
        }
        truncated.push_str(grapheme);
        current_width += grapheme_width;
    }

    truncated + "…"
}

pub fn with_padding(text: &str, padding: usize, alignment: Alignment) -> String {
    let uncolored = crate::utils::color::decolorise(text);
    let width = string_width(&uncolored);
    if padding < width {
        return text.to_string();
    }
    let space = " ".repeat(padding - width);
    match alignment {
        Alignment::Left => format!("{}{}", text, space),
        Alignment::Right => format!("{}{}", space, text),
    }
}

pub fn safe_truncate(text: &str, limit: usize) -> String {
    if text.len() > limit {
        text[..limit].to_string()
    } else {
        text.to_string()
    }
}

pub const COMMIT_HASH_SHORT_SIZE: usize = 8;

pub fn short_hash(hash: &str) -> String {
    if hash.len() < COMMIT_HASH_SHORT_SIZE {
        hash.to_string()
    } else {
        hash[..COMMIT_HASH_SHORT_SIZE].to_string()
    }
}

pub fn format_paths(paths: &[String]) -> String {
    if paths.len() <= 3 {
        return paths.join(", ");
    }
    format!(
        "{}, {}, {}, [...{} more]",
        paths[0],
        paths[1],
        paths[2],
        paths.len() - 3
    )
}

fn max_fn<T, F>(items: &[T], f: F) -> usize
where
    F: Fn(&T) -> usize,
{
    let mut max = 0;
    for item in items {
        let val = f(item);
        if val > max {
            max = val;
        }
    }
    max
}

fn get_pad_widths(string_arrays: &[Vec<String>]) -> Vec<usize> {
    let max_width = max_fn(string_arrays, |arr| arr.len());

    if max_width.saturating_sub(1) == 0 {
        return vec![];
    }

    (0..max_width.saturating_sub(1))
        .map(|i| {
            max_fn(string_arrays, |arr| {
                let s = arr.get(i).map(|s| s.as_str()).unwrap_or("");
                crate::utils::color::decolorise(s);
                string_width(&crate::utils::color::decolorise(s))
            })
        })
        .collect()
}

fn get_padded_display_strings(
    string_arrays: &[Vec<String>],
    column_configs: &[ColumnConfig],
) -> Vec<String> {
    let mut result = Vec::with_capacity(string_arrays.len());

    for string_array in string_arrays {
        if string_array.is_empty() {
            continue;
        }

        let mut builder = String::new();
        for (j, config) in column_configs.iter().enumerate() {
            if string_array.len().saturating_sub(1) < j {
                continue;
            }
            builder.push_str(&with_padding(
                &string_array[j],
                config.width,
                config.alignment,
            ));
            builder.push(' ');
        }

        if string_array.len().saturating_sub(1) < column_configs.len() {
            continue;
        }

        builder.push_str(&string_array[column_configs.len()]);
        result.push(builder);
    }

    result
}

fn exclude_blank_columns(
    display_strings_arr: &[Vec<String>],
    column_alignments: &[Alignment],
) -> (Vec<Vec<String>>, Vec<Alignment>, Vec<usize>) {
    if display_strings_arr.is_empty() {
        return (
            display_strings_arr.to_vec(),
            column_alignments.to_vec(),
            vec![],
        );
    }

    let mut to_remove = Vec::new();
    'outer: for i in 0..display_strings_arr[0].len() {
        for strings in display_strings_arr {
            if !strings[i].is_empty() {
                continue 'outer;
            }
        }
        to_remove.push(i);
    }

    if to_remove.is_empty() {
        return (
            display_strings_arr.to_vec(),
            column_alignments.to_vec(),
            vec![],
        );
    }

    let result: Vec<Vec<String>> = display_strings_arr
        .iter()
        .map(|strings| {
            let mut kept = strings.clone();
            for &idx in to_remove.iter().rev() {
                if idx < kept.len() {
                    kept.remove(idx);
                }
            }
            kept
        })
        .collect();

    let mut alignments = column_alignments.to_vec();
    for &idx in to_remove.iter().rev() {
        if idx < alignments.len() {
            alignments.remove(idx);
        }
    }

    (result, alignments, to_remove)
}

pub fn render_display_strings(
    display_strings_arr: &[Vec<String>],
    column_alignments: &[Alignment],
) -> (Vec<String>, Vec<usize>) {
    if display_strings_arr.is_empty() {
        return (vec![], vec![]);
    }

    let (display_strings_arr, column_alignments, removed_columns) =
        exclude_blank_columns(display_strings_arr, column_alignments);
    let pad_widths = get_pad_widths(&display_strings_arr);

    let column_configs: Vec<ColumnConfig> = pad_widths
        .iter()
        .enumerate()
        .map(|(i, &width)| ColumnConfig {
            width,
            alignment: column_alignments.get(i).copied().unwrap_or(Alignment::Left),
        })
        .collect();

    let mut column_positions = vec![0usize; pad_widths.len() + 1];
    column_positions[0] = 0;
    for (i, &width) in pad_widths.iter().enumerate() {
        column_positions[i + 1] = column_positions[i] + width + 1;
    }

    for &removed_column in &removed_columns {
        if removed_column < column_positions.len() {
            column_positions.insert(removed_column, column_positions[removed_column]);
        }
    }

    (
        get_padded_display_strings(&display_strings_arr, &column_configs),
        column_positions,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_width_ascii() {
        assert_eq!(string_width("hello"), 5);
        assert_eq!(string_width(""), 0);
        assert_eq!(string_width("test"), 4);
    }

    #[test]
    fn test_string_width_unicode() {
        // UnicodeWidthStr::width returns display width: CJK chars are 2 cells each
        assert_eq!(string_width("日本"), 4);
        assert_eq!(string_width("中文"), 4);
    }

    #[test]
    fn test_with_padding_left() {
        assert_eq!(with_padding("hi", 5, Alignment::Left), "hi   ");
        assert_eq!(with_padding("hello", 5, Alignment::Left), "hello");
    }

    #[test]
    fn test_with_padding_right() {
        assert_eq!(with_padding("hi", 5, Alignment::Right), "   hi");
        assert_eq!(with_padding("hello", 5, Alignment::Right), "hello");
    }

    #[test]
    fn test_truncate_with_ellipsis() {
        assert_eq!(truncate_with_ellipsis("hello", 10), "hello");
        assert_eq!(truncate_with_ellipsis("hello", 3), "he…");
        assert_eq!(truncate_with_ellipsis("hi", 2), "hi");
    }

    #[test]
    fn test_safe_truncate() {
        assert_eq!(safe_truncate("hello", 3), "hel");
        assert_eq!(safe_truncate("hi", 10), "hi");
    }

    #[test]
    fn test_short_hash() {
        assert_eq!(short_hash("abc"), "abc");
        assert_eq!(short_hash("1234567890abcdef"), "12345678");
    }

    #[test]
    fn test_format_paths() {
        assert_eq!(format_paths(&["a".to_string()]), "a");
        assert_eq!(
            format_paths(&["a".to_string(), "b".to_string(), "c".to_string()]),
            "a, b, c"
        );
        assert_eq!(
            format_paths(&[
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string()
            ]),
            "a, b, c, [...1 more]"
        );
    }

    #[test]
    fn test_render_display_strings() {
        let strings = vec![vec!["a".to_string(), "b".to_string()]];
        let (result, positions) = render_display_strings(&strings, &[Alignment::Left]);
        assert!(!result.is_empty());
        assert_eq!(positions.len(), 2);
    }

    #[test]
    fn test_render_display_strings_empty() {
        let (result, positions) = render_display_strings(&[], &[Alignment::Left]);
        assert!(result.is_empty());
        assert!(positions.is_empty());
    }
}
