use regex::Regex;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMenuItem {
    pub label: String,
    pub value: String,
}

pub struct MenuGenerator;

impl MenuGenerator {
    pub fn call(
        command_output: &str,
        filter: &str,
        value_format: &str,
        label_format: &str,
    ) -> Result<Vec<CommandMenuItem>, String> {
        let menu_item_from_line = Self::build_line_parser(filter, value_format, label_format)?;

        let mut items = Vec::new();
        for line in command_output.split('\n') {
            if line.is_empty() {
                continue;
            }
            let item = menu_item_from_line(line)?;
            items.push(item);
        }
        Ok(items)
    }

    fn build_line_parser(
        filter: &str,
        value_format: &str,
        label_format: &str,
    ) -> Result<Box<dyn Fn(&str) -> Result<CommandMenuItem, String>>, String> {
        if filter.is_empty() && value_format.is_empty() && label_format.is_empty() {
            return Ok(Box::new(|line: &str| {
                Ok(CommandMenuItem {
                    label: line.to_string(),
                    value: line.to_string(),
                })
            }));
        }

        let regex =
            Regex::new(filter).map_err(|e| format!("unable to parse filter regex, error: {e}"))?;

        let value_fmt = value_format.to_string();
        let label_fmt = if label_format.is_empty() {
            value_format.to_string()
        } else {
            label_format.to_string()
        };

        let group_names: Vec<String> = regex
            .capture_names()
            .enumerate()
            .map(|(idx, name)| (idx, name.map(String::from).unwrap_or_default()))
            .map(|(_, name)| name)
            .collect();

        let _ = group_names;

        let regex_clone = regex.clone();
        let group_names_for_closure: Vec<Option<String>> =
            regex.capture_names().map(|n| n.map(String::from)).collect();

        Ok(Box::new(move |line: &str| {
            let tmpl_data = parse_line(line, &regex_clone, &group_names_for_closure);

            let value = execute_template(&value_fmt, &tmpl_data)
                .map_err(|e| format!("value template error: {e}"))?;
            let label = execute_template(&label_fmt, &tmpl_data)
                .map_err(|e| format!("label template error: {e}"))?;

            Ok(CommandMenuItem { label, value })
        }))
    }
}

fn parse_line(
    line: &str,
    regex: &Regex,
    group_names: &[Option<String>],
) -> HashMap<String, String> {
    let mut tmpl_data = HashMap::new();

    if let Some(captures) = regex.captures(line) {
        for (group_idx, name) in group_names.iter().enumerate() {
            if let Some(matched) = captures.get(group_idx) {
                let match_name = format!("group_{group_idx}");
                tmpl_data.insert(match_name, matched.as_str().to_string());
                if let Some(named) = name {
                    if !named.is_empty() {
                        tmpl_data.insert(named.clone(), matched.as_str().to_string());
                    }
                }
            }
        }
    }

    tmpl_data
}

fn execute_template(format_str: &str, data: &HashMap<String, String>) -> Result<String, String> {
    let mut result = format_str.to_string();
    for (key, value) in data {
        let placeholder = format!("{{{{{key}}}}}");
        let dot_placeholder = format!("{{{{.{key}}}}}");
        result = result.replace(&placeholder, value);
        result = result.replace(&dot_placeholder, value);
    }
    Ok(result.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_with_no_filter_returns_lines_as_is() {
        let items = MenuGenerator::call("alpha\nbeta\ngamma", "", "", "").unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].label, "alpha");
        assert_eq!(items[0].value, "alpha");
        assert_eq!(items[2].label, "gamma");
    }

    #[test]
    fn call_skips_empty_lines() {
        let items = MenuGenerator::call("alpha\n\nbeta\n", "", "", "").unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn call_with_regex_filter_and_template() {
        let items = MenuGenerator::call(
            "abc 123\ndef 456",
            r"(?P<word>\w+)\s+(?P<num>\d+)",
            "{{.word}}",
            "{{.num}}: {{.word}}",
        )
        .unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].value, "abc");
        assert_eq!(items[0].label, "123: abc");
        assert_eq!(items[1].value, "def");
        assert_eq!(items[1].label, "456: def");
    }

    #[test]
    fn call_with_group_index_references() {
        let items = MenuGenerator::call(
            "hello world",
            r"(\w+)\s+(\w+)",
            "{{.group_1}}-{{.group_2}}",
            "",
        )
        .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].value, "hello-world");
        assert_eq!(items[0].label, "hello-world");
    }

    #[test]
    fn call_with_invalid_regex_returns_error() {
        let result = MenuGenerator::call("test", "[invalid", "", "");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unable to parse filter regex"));
    }

    #[test]
    fn label_format_defaults_to_value_format_when_empty() {
        let items =
            MenuGenerator::call("abc 123", r"(?P<word>\w+)\s+(?P<num>\d+)", "{{.word}}", "")
                .unwrap();
        assert_eq!(items[0].label, items[0].value);
    }
}
