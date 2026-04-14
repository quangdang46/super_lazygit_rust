use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn split_lines(s: &str) -> Vec<String> {
    let s = s.replace('\r', "");
    if s.is_empty() || s == "\n" {
        return vec![];
    }
    let lines: Vec<&str> = s.split('\n').collect();
    if lines[lines.len() - 1].is_empty() {
        lines[..lines.len() - 1]
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        lines.iter().map(|s| s.to_string()).collect()
    }
}

pub fn split_nul(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return vec![];
    }
    let s = s.strip_suffix('\x00').unwrap_or(s);
    s.split('\x00').collect()
}

pub fn normalize_linefeeds(s: &str) -> String {
    let s = s.replace("\r\n", "\n");
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\r' {
            if i + 1 < chars.len() && chars[i + 1] == '\n' {
                result.push('\n');
                i += 2;
            } else {
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

pub fn escape_special_chars(s: &str) -> String {
    let mut result = s.to_string();
    result = result.replace('\n', "\\n");
    result = result.replace('\r', "\\r");
    result = result.replace('\t', "\\t");
    result = result.replace('\u{08}', "\\b");
    result = result.replace('\u{0C}', "\\f");
    result = result.replace('\u{0B}', "\\v");
    result
}

fn drop_cr(data: &[u8]) -> &[u8] {
    if !data.is_empty() && data[data.len() - 1] == b'\r' {
        &data[..data.len() - 1]
    } else {
        data
    }
}

pub fn scan_lines_and_truncate_when_longer_than_buffer(
    max_buffer_size: usize,
) -> impl FnMut(&[u8], bool) -> Result<usize, std::io::Error> {
    let mut skip_over_remainder_of_long_line = false;

    move |data: &[u8], at_eof: bool| {
        if at_eof && data.is_empty() {
            return Ok(0);
        }
        if let Some(i) = data.iter().position(|&b| b == b'\n') {
            if skip_over_remainder_of_long_line {
                skip_over_remainder_of_long_line = false;
                return Ok(i + 1);
            }
            return Ok(i + 1);
        }
        if at_eof {
            if skip_over_remainder_of_long_line {
                return Ok(data.len());
            }
            return Ok(drop_cr(data).len());
        }

        if data.len() >= max_buffer_size {
            if skip_over_remainder_of_long_line {
                return Ok(data.len());
            }
            skip_over_remainder_of_long_line = true;
            return Ok(data.len());
        }

        Ok(0)
    }
}

pub fn wrap_view_lines_to_width(
    wrap: bool,
    editable: bool,
    text: &str,
    width: usize,
    tab_width: usize,
) -> (Vec<String>, Vec<usize>, Vec<usize>) {
    let text = if !editable {
        text.strip_suffix('\n').unwrap_or(text)
    } else {
        text
    };
    let lines: Vec<&str> = text.split('\n').collect();
    if !wrap {
        let indices: Vec<usize> = (0..lines.len()).collect();
        let original_indices = indices.clone();
        return (
            lines.iter().map(|s| s.to_string()).collect(),
            indices,
            original_indices,
        );
    }

    let mut wrapped_lines: Vec<String> = Vec::new();
    let mut wrapped_line_indices: Vec<usize> = Vec::new();
    let mut original_line_indices: Vec<usize> = Vec::new();

    let tab_width = if tab_width < 1 { 4 } else { tab_width };

    for (original_line_idx, line) in lines.iter().enumerate() {
        wrapped_line_indices.push(wrapped_lines.len());

        let line = line.replace('\t', &" ".repeat(tab_width));
        let line_bytes = line.as_bytes();

        let mut n = 0;
        let mut offset = 0;
        let mut last_whitespace_index: Option<usize> = None;

        for (i, curr_chr) in line.char_indices() {
            let rw = UnicodeWidthChar::width(curr_chr).unwrap_or(0);
            n += rw;

            if n > width {
                if curr_chr == ' ' {
                    wrapped_lines.push(line[offset..i].to_string());
                    original_line_indices.push(original_line_idx);
                    offset = i + 1;
                    n = 0;
                } else if curr_chr == '-' {
                    wrapped_lines.push(line[offset..i].to_string());
                    original_line_indices.push(original_line_idx);
                    offset = i;
                    n = rw;
                } else if let Some(lwi) = last_whitespace_index {
                    if line_bytes[lwi] == b'-' {
                        wrapped_lines.push(line[offset..=lwi].to_string());
                    } else {
                        wrapped_lines.push(line[offset..lwi].to_string());
                    }
                    original_line_indices.push(original_line_idx);
                    offset = lwi + 1;
                    n = UnicodeWidthStr::width(&line[offset..=i]);
                } else {
                    wrapped_lines.push(line[offset..i].to_string());
                    original_line_indices.push(original_line_idx);
                    offset = i;
                    n = rw;
                }
                last_whitespace_index = None;
            } else if curr_chr == ' ' || curr_chr == '-' {
                last_whitespace_index = Some(i);
            }
        }

        wrapped_lines.push(line[offset..].to_string());
        original_line_indices.push(original_line_idx);
    }

    (wrapped_lines, wrapped_line_indices, original_line_indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_lines() {
        assert_eq!(split_lines("a\nb\nc"), vec!["a", "b", "c"]);
        assert_eq!(split_lines("a\nb\n"), vec!["a", "b"]);
        assert_eq!(split_lines("a\r\nb\r\nc"), vec!["a", "b", "c"]);
        assert_eq!(split_lines(""), Vec::<String>::new());
        assert_eq!(split_lines("\n"), Vec::<String>::new());
    }

    #[test]
    fn test_split_nul() {
        assert_eq!(split_nul("a\x00b\x00c"), vec!["a", "b", "c"]);
        assert_eq!(split_nul("a\x00b\x00"), vec!["a", "b"]);
        assert_eq!(split_nul(""), Vec::<&str>::new());
    }

    #[test]
    fn test_normalize_linefeeds() {
        assert_eq!(normalize_linefeeds("a\nb\nc"), "a\nb\nc");
        assert_eq!(normalize_linefeeds("a\r\nb\r\nc"), "a\nb\nc");
        assert_eq!(normalize_linefeeds("a\rb\rc"), "abc");
    }

    #[test]
    fn test_escape_special_chars() {
        assert_eq!(escape_special_chars("a\nb"), "a\\nb");
        assert_eq!(escape_special_chars("a\rb"), "a\\rb");
        assert_eq!(escape_special_chars("a\tb"), "a\\tb");
    }

    #[test]
    fn test_wrap_view_lines_to_width_no_wrap() {
        let (lines, wrapped_idx, orig_idx) =
            wrap_view_lines_to_width(false, true, "a\nb\nc", 10, 4);
        assert_eq!(lines, vec!["a", "b", "c"]);
        assert_eq!(wrapped_idx, vec![0, 1, 2]);
        assert_eq!(orig_idx, vec![0, 1, 2]);
    }

    #[test]
    fn test_wrap_view_lines_to_width_with_wrap() {
        let (lines, wrapped_idx, orig_idx) =
            wrap_view_lines_to_width(true, true, "hello world", 5, 4);
        assert!(lines.len() > 1);
        assert_eq!(wrapped_idx, vec![0]);
        assert_eq!(orig_idx.len(), lines.len());
    }
}
