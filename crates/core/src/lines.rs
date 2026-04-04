use unicode_width::UnicodeWidthChar;

#[must_use]
pub fn split_lines(multiline_string: &str) -> Vec<String> {
    let multiline_string = multiline_string.replace('\r', "");
    if multiline_string.is_empty() || multiline_string == "\n" {
        return Vec::new();
    }

    let mut lines = multiline_string
        .split('\n')
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

#[must_use]
pub fn split_nul(value: &str) -> Vec<String> {
    if value.is_empty() {
        return Vec::new();
    }

    value
        .strip_suffix('\0')
        .unwrap_or(value)
        .split('\0')
        .map(ToString::to_string)
        .collect()
}

#[must_use]
pub fn normalize_linefeeds(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "")
}

#[must_use]
pub fn escape_special_chars(value: &str) -> String {
    value
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
        .replace('\u{0008}', "\\b")
        .replace('\u{000c}', "\\f")
        .replace('\u{000b}', "\\v")
}

fn drop_cr(data: &[u8]) -> &[u8] {
    if data.last() == Some(&b'\r') {
        &data[..data.len() - 1]
    } else {
        data
    }
}

fn scan_lines_chunk(
    data: &[u8],
    at_eof: bool,
    max_buffer_size: usize,
    skip_over_remainder_of_long_line: &mut bool,
) -> (usize, Option<String>) {
    if at_eof && data.is_empty() {
        return (0, None);
    }

    if let Some(newline_index) = data.iter().position(|byte| *byte == b'\n') {
        if *skip_over_remainder_of_long_line {
            *skip_over_remainder_of_long_line = false;
            return (newline_index + 1, None);
        }

        return (
            newline_index + 1,
            Some(String::from_utf8_lossy(drop_cr(&data[..newline_index])).into_owned()),
        );
    }

    if at_eof {
        if *skip_over_remainder_of_long_line {
            return (data.len(), None);
        }

        return (
            data.len(),
            Some(String::from_utf8_lossy(drop_cr(data)).into_owned()),
        );
    }

    if data.len() >= max_buffer_size {
        if *skip_over_remainder_of_long_line {
            return (data.len(), None);
        }

        *skip_over_remainder_of_long_line = true;
        return (data.len(), Some(String::from_utf8_lossy(data).into_owned()));
    }

    (0, None)
}

#[must_use]
pub fn scan_lines_and_truncate_when_longer_than_buffer(
    input: &str,
    max_buffer_size: usize,
) -> Vec<String> {
    if max_buffer_size == 0 {
        return Vec::new();
    }

    let bytes = input.as_bytes();
    let mut cursor = 0usize;
    let mut lines = Vec::new();
    let mut skip_over_remainder_of_long_line = false;

    while cursor < bytes.len() {
        let end = (cursor + max_buffer_size).min(bytes.len());
        let at_eof = end == bytes.len();
        let (advance, token) = scan_lines_chunk(
            &bytes[cursor..end],
            at_eof,
            max_buffer_size,
            &mut skip_over_remainder_of_long_line,
        );
        if let Some(token) = token {
            lines.push(token);
        }

        if advance == 0 {
            break;
        }

        cursor += advance;
    }

    lines
}

fn expand_tabs(line: &str, tab_width: usize) -> String {
    let mut expanded = String::with_capacity(line.len());
    let mut column = 0usize;

    for ch in line.chars() {
        if ch == '\t' {
            let num_spaces = tab_width - (column % tab_width);
            expanded.extend(std::iter::repeat_n(' ', num_spaces));
            column += num_spaces;
        } else {
            expanded.push(ch);
            column += 1;
        }
    }

    expanded
}

fn string_width(value: &str) -> usize {
    value
        .chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
}

#[must_use]
pub fn wrap_view_lines_to_width(
    wrap: bool,
    editable: bool,
    text: &str,
    width: usize,
    tab_width: usize,
) -> (Vec<String>, Vec<usize>, Vec<usize>) {
    let text = if editable {
        text.to_string()
    } else {
        text.trim_end_matches('\n').to_string()
    };
    let lines = text.split('\n').collect::<Vec<_>>();

    if !wrap {
        let indices = (0..lines.len()).collect::<Vec<_>>();
        return (
            lines.into_iter().map(ToString::to_string).collect(),
            indices.clone(),
            indices,
        );
    }

    let mut wrapped_lines = Vec::with_capacity(lines.len());
    let mut wrapped_line_indices = Vec::with_capacity(lines.len());
    let mut original_line_indices = Vec::with_capacity(lines.len());
    let tab_width = tab_width.max(1);

    for (original_line_idx, line) in lines.iter().enumerate() {
        wrapped_line_indices.push(wrapped_lines.len());
        let line = expand_tabs(line, tab_width);

        let mut append_wrapped_line = |value: &str| {
            wrapped_lines.push(value.to_string());
            original_line_indices.push(original_line_idx);
        };

        let mut current_width = 0usize;
        let mut offset = 0usize;
        let mut last_whitespace_index = None;

        for (index, curr_chr) in line.char_indices() {
            let rune_width = UnicodeWidthChar::width(curr_chr).unwrap_or(0);
            current_width += rune_width;

            if current_width > width {
                if curr_chr == ' ' {
                    append_wrapped_line(&line[offset..index]);
                    offset = index + curr_chr.len_utf8();
                    current_width = 0;
                } else if curr_chr == '-' {
                    append_wrapped_line(&line[offset..index]);
                    offset = index;
                    current_width = rune_width;
                } else if let Some(last_index) = last_whitespace_index {
                    if line[last_index..].starts_with('-') {
                        append_wrapped_line(&line[offset..last_index + 1]);
                    } else {
                        append_wrapped_line(&line[offset..last_index]);
                    }
                    offset = last_index + 1;
                    current_width = string_width(&line[offset..index + curr_chr.len_utf8()]);
                } else {
                    append_wrapped_line(&line[offset..index]);
                    offset = index;
                    current_width = rune_width;
                }
                last_whitespace_index = None;
            } else if curr_chr == ' ' || curr_chr == '-' {
                last_whitespace_index = Some(index);
            }
        }

        append_wrapped_line(&line[offset..]);
    }

    (wrapped_lines, wrapped_line_indices, original_line_indices)
}

#[cfg(test)]
mod tests {
    use super::{
        escape_special_chars, normalize_linefeeds, scan_lines_and_truncate_when_longer_than_buffer,
        split_lines, split_nul, wrap_view_lines_to_width,
    };

    #[test]
    fn split_lines_matches_upstream_cases() {
        let scenarios = [
            ("", Vec::<String>::new()),
            ("\n", Vec::<String>::new()),
            (
                "hello world !\nhello universe !\n",
                vec!["hello world !".to_string(), "hello universe !".to_string()],
            ),
        ];

        for (input, expected) in scenarios {
            assert_eq!(split_lines(input), expected);
        }
    }

    #[test]
    fn split_nul_matches_upstream_cases() {
        let scenarios = [
            ("", Vec::<String>::new()),
            ("\0", vec!["".to_string()]),
            (
                "hello world !\0hello universe !\0",
                vec!["hello world !".to_string(), "hello universe !".to_string()],
            ),
        ];

        for (input, expected) in scenarios {
            assert_eq!(split_nul(input), expected);
        }
    }

    #[test]
    fn normalize_linefeeds_matches_upstream_cases() {
        let scenarios = [
            ("asdf\r\n", "asdf\n"),
            ("asdf\r\nasdf", "asdf\nasdf"),
            ("asdf\r", "asdf"),
            ("asdf\n", "asdf\n"),
        ];

        for (input, expected) in scenarios {
            assert_eq!(normalize_linefeeds(input), expected);
        }
    }

    #[test]
    fn escape_special_chars_matches_go_replacements() {
        assert_eq!(
            escape_special_chars("a\nb\rc\td\u{0008}e\u{000c}f\u{000b}g"),
            r"a\nb\rc\td\be\ff\vg"
        );
    }

    #[test]
    fn scan_lines_and_truncate_matches_upstream_cases() {
        let scenarios = [
            ("", Vec::<String>::new()),
            ("\n", vec!["".to_string()]),
            ("abc", vec!["abc".to_string()]),
            ("abc\ndef", vec!["abc".to_string(), "def".to_string()]),
            (
                "abc\n\ndef",
                vec!["abc".to_string(), "".to_string(), "def".to_string()],
            ),
            ("abc\r\ndef\r", vec!["abc".to_string(), "def".to_string()]),
            ("abcdef", vec!["abcde".to_string()]),
            ("abcdef\n", vec!["abcde".to_string()]),
            (
                "abcdef\nghijkl\nx",
                vec!["abcde".to_string(), "ghijk".to_string(), "x".to_string()],
            ),
            (
                "abc\ndefghijklmnopqrstuvw\nx",
                vec!["abc".to_string(), "defgh".to_string(), "x".to_string()],
            ),
        ];

        for (input, expected) in scenarios {
            assert_eq!(
                scan_lines_and_truncate_when_longer_than_buffer(input, 5),
                expected
            );
        }
    }

    #[test]
    fn wrap_view_lines_to_width_matches_upstream_cases() {
        let tests = vec![
            (
                false,
                false,
                "1st line\n2nd line\n3rd line",
                5,
                4,
                vec!["1st line", "2nd line", "3rd line"],
                vec![0, 1, 2],
                vec![0, 1, 2],
            ),
            (
                true,
                false,
                "Hello World",
                5,
                4,
                vec!["Hello", "World"],
                vec![0],
                vec![0, 0],
            ),
            (
                true,
                false,
                "Hello-World",
                6,
                4,
                vec!["Hello-", "World"],
                vec![0],
                vec![0, 0],
            ),
            (
                true,
                false,
                "Blah Hello-World",
                12,
                4,
                vec!["Blah Hello-", "World"],
                vec![0],
                vec![0, 0],
            ),
            (
                true,
                false,
                "Blah Hello-World",
                11,
                4,
                vec!["Blah Hello-", "World"],
                vec![0],
                vec![0, 0],
            ),
            (
                true,
                false,
                "Blah Hello-World",
                10,
                4,
                vec!["Blah Hello", "-World"],
                vec![0],
                vec![0, 0],
            ),
            (
                true,
                false,
                "Blah Hello World",
                10,
                4,
                vec!["Blah Hello", "World"],
                vec![0],
                vec![0, 0],
            ),
            (
                true,
                false,
                "Longer word here",
                10,
                4,
                vec!["Longer", "word here"],
                vec![0],
                vec![0, 0],
            ),
            (
                true,
                false,
                "ThisWordIsWayTooLong",
                10,
                4,
                vec!["ThisWordIs", "WayTooLong"],
                vec![0],
                vec![0, 0],
            ),
            (
                true,
                false,
                "ThisWordIsWayTooLong",
                5,
                4,
                vec!["ThisW", "ordIs", "WayTo", "oLong"],
                vec![0],
                vec![0, 0, 0, 0],
            ),
            (
                true,
                false,
                "one-two-three-four-five",
                8,
                4,
                vec!["one-two-", "three-", "four-", "five"],
                vec![0],
                vec![0, 0, 0, 0],
            ),
            (
                true,
                false,
                "aaa bb cc ddd-ee ff",
                5,
                4,
                vec!["aaa", "bb cc", "ddd-", "ee ff"],
                vec![0],
                vec![0, 0, 0, 0],
            ),
            (
                true,
                false,
                "🐤🐤🐤 🐝🐝 🙉🙉 🦊🦊🦊-🐬🐬 🦢🦢",
                9,
                4,
                vec!["🐤🐤🐤", "🐝🐝 🙉🙉", "🦊🦊🦊-", "🐬🐬 🦢🦢"],
                vec![0],
                vec![0, 0, 0, 0],
            ),
            (
                true,
                false,
                "hello world",
                6,
                4,
                vec!["hello", "world"],
                vec![0],
                vec![0, 0],
            ),
            (
                true,
                false,
                "hello-world",
                6,
                4,
                vec!["hello-", "world"],
                vec![0],
                vec![0, 0],
            ),
            (
                true,
                false,
                "+The sea reach of the Thames stretched before us like the bedinnind of an interminable waterway. In the offind the sea and the sky were welded todether without a joint, and in the luminous space the tanned sails of the bardes drifting blah blah",
                81,
                4,
                vec![
                    "+The sea reach of the Thames stretched before us like the bedinnind of an",
                    "interminable waterway. In the offind the sea and the sky were welded todether",
                    "without a joint, and in the luminous space the tanned sails of the bardes",
                    "drifting blah blah",
                ],
                vec![0],
                vec![0, 0, 0, 0],
            ),
            (
                true,
                false,
                "\ta\tbb\tccc\tdddd\teeeee",
                50,
                4,
                vec!["    a   bb  ccc dddd    eeeee"],
                vec![0],
                vec![0],
            ),
            (
                true,
                false,
                "\ta\tbb\tccc\tdddddddd\teeeee",
                100,
                8,
                vec!["        a       bb      ccc     dddddddd        eeeee"],
                vec![0],
                vec![0],
            ),
            (
                true,
                false,
                "First paragraph\nThe second paragraph is a bit longer.\nThird paragraph\n",
                10,
                4,
                vec![
                    "First",
                    "paragraph",
                    "The second",
                    "paragraph",
                    "is a bit",
                    "longer.",
                    "Third",
                    "paragraph",
                ],
                vec![0, 2, 6],
                vec![0, 0, 1, 1, 1, 1, 2, 2],
            ),
            (
                true,
                false,
                "First\nSecond\nThird\n",
                10,
                4,
                vec!["First", "Second", "Third"],
                vec![0, 1, 2],
                vec![0, 1, 2],
            ),
            (
                true,
                true,
                "First\nSecond\nThird\n",
                10,
                4,
                vec!["First", "Second", "Third", ""],
                vec![0, 1, 2, 3],
                vec![0, 1, 2, 3],
            ),
        ];

        for (
            wrap,
            editable,
            text,
            width,
            tab_width,
            expected_lines,
            expected_wrapped_indices,
            expected_original_indices,
        ) in tests
        {
            let (wrapped_lines, wrapped_line_indices, original_line_indices) =
                wrap_view_lines_to_width(wrap, editable, text, width, tab_width);
            assert_eq!(wrapped_lines, expected_lines);
            assert_eq!(wrapped_line_indices, expected_wrapped_indices);
            assert_eq!(original_line_indices, expected_original_indices);
        }
    }
}
