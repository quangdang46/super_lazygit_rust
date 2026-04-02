use std::fmt;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangesSet {
    changes: Vec<String>,
}

impl ChangesSet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, change: impl Into<String>) {
        let change = change.into();
        if !self.changes.iter().any(|existing| existing == &change) {
            self.changes.push(change);
        }
    }

    #[must_use]
    pub fn to_slice_from_oldest(&self) -> Vec<String> {
        self.changes.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationError(String);

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MigrationError {}

pub fn compute_migrated_config(
    _path: &str,
    content: &[u8],
    changes: &mut ChangesSet,
) -> Result<(Vec<u8>, bool), MigrationError> {
    let input = String::from_utf8(content.to_vec())
        .map_err(|error| MigrationError(format!("failed to parse YAML: {error}")))?;
    if input.is_empty() {
        return Ok((Vec::new(), false));
    }

    let mut lines = input.lines().map(ToString::to_string).collect::<Vec<_>>();
    let mut did_change = false;

    did_change |= rename_keys(&mut lines, changes);
    did_change |= change_null_keybindings_to_disabled(&mut lines, changes);
    did_change |= change_commit_prefix_to_sequence(&mut lines, changes);
    did_change |= change_commit_prefixes_map(&mut lines, changes);
    did_change |= change_custom_command_output_flags(&mut lines, changes);
    did_change |= migrate_all_branches_log_cmd(&mut lines, changes);
    did_change |= migrate_pagers(&mut lines, changes);

    if !did_change {
        return Ok((Vec::new(), false));
    }

    let mut output = lines.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    Ok((output.into_bytes(), true))
}

fn rename_keys(lines: &mut [String], changes: &mut ChangesSet) -> bool {
    let mut changed = false;
    changed |= rename_scalar_key(
        lines,
        "skipUnstageLineWarning",
        "skipDiscardChangeWarning",
        "Renamed 'gui.skipUnstageLineWarning' to 'skipDiscardChangeWarning'",
        changes,
    );
    changed |= rename_scalar_key(
        lines,
        "executeCustomCommand",
        "executeShellCommand",
        "Renamed 'keybinding.universal.executeCustomCommand' to 'executeShellCommand'",
        changes,
    );
    changed |= rename_scalar_key(
        lines,
        "windowSize",
        "screenMode",
        "Renamed 'gui.windowSize' to 'screenMode'",
        changes,
    );
    changed
}

fn rename_scalar_key(
    lines: &mut [String],
    old_key: &str,
    new_key: &str,
    message: &str,
    changes: &mut ChangesSet,
) -> bool {
    let mut changed = false;
    for line in lines {
        let trimmed = line.trim_start();
        let prefix = format!("{old_key}:");
        if trimmed.starts_with(&prefix) {
            let indent = line.len() - trimmed.len();
            let suffix = &trimmed[prefix.len()..];
            *line = format!("{}{}:{}", " ".repeat(indent), new_key, suffix);
            changed = true;
        }
    }
    if changed {
        changes.add(message);
    }
    changed
}

fn change_null_keybindings_to_disabled(lines: &mut [String], changes: &mut ChangesSet) -> bool {
    let mut changed = false;
    let mut in_keybinding = false;
    let mut keybinding_indent = 0usize;
    let mut stack: Vec<(usize, String)> = Vec::new();

    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - trimmed.len();

        if trimmed == "keybinding:" {
            in_keybinding = true;
            keybinding_indent = indent;
            stack.clear();
            continue;
        }

        if in_keybinding && indent <= keybinding_indent {
            in_keybinding = false;
            stack.clear();
        }
        if !in_keybinding {
            continue;
        }

        while stack
            .last()
            .is_some_and(|(stack_indent, _)| indent <= *stack_indent)
        {
            stack.pop();
        }

        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim().to_string();
        let value = value.trim();
        if value.is_empty() {
            stack.push((indent, key));
            continue;
        }
        if value == "null" {
            *line = format!("{}{}: <disabled>", " ".repeat(indent), key);
            let path = stack
                .iter()
                .map(|(_, segment)| segment.as_str())
                .chain(std::iter::once(key.as_str()))
                .collect::<Vec<_>>()
                .join(".");
            changes.add(format!(
                "Changed 'null' to '<disabled>' for keybinding 'keybinding.{path}'"
            ));
            changed = true;
        }
    }

    changed
}

fn change_commit_prefix_to_sequence(lines: &mut Vec<String>, changes: &mut ChangesSet) -> bool {
    let mut index = 0usize;
    while index < lines.len() {
        if lines[index].trim() == "commitPrefix:"
            && convert_mapping_block_to_sequence(lines, index).is_some()
        {
            changes.add("Changed 'git.commitPrefix' to an array of strings");
            return true;
        }
        index += 1;
    }
    false
}

fn change_commit_prefixes_map(lines: &mut Vec<String>, changes: &mut ChangesSet) -> bool {
    let Some(start) = lines
        .iter()
        .position(|line| line.trim() == "commitPrefixes:")
    else {
        return false;
    };
    let base_indent = indent_of(&lines[start]);
    let mut changed = false;
    let mut index = start + 1;
    while index < lines.len() {
        let trimmed = lines[index].trim_start();
        let indent = indent_of(&lines[index]);
        if !trimmed.is_empty() && indent <= base_indent {
            break;
        }
        if trimmed.ends_with(':')
            && !trimmed.starts_with('-')
            && indent > base_indent
            && convert_mapping_block_to_sequence(lines, index).is_some()
        {
            changed = true;
        }
        index += 1;
    }

    if changed {
        changes.add("Changed 'git.commitPrefixes' elements to arrays of strings");
    }
    changed
}

fn change_custom_command_output_flags(lines: &mut Vec<String>, changes: &mut ChangesSet) -> bool {
    let Some(start) = lines
        .iter()
        .position(|line| line.trim() == "customCommands:")
    else {
        return false;
    };
    let base_indent = indent_of(&lines[start]);
    let mut changed = false;
    let mut index = start + 1;

    while index < lines.len() {
        let trimmed = lines[index].trim_start();
        let indent = indent_of(&lines[index]);
        if !trimmed.is_empty() && indent <= base_indent {
            break;
        }
        if trimmed.starts_with("- ") && indent == base_indent + 2 {
            let end = next_sequence_or_dedent(lines, index + 1, indent);
            let item = process_custom_command_item(&lines[index..end], indent, changes);
            if let Some(item) = item {
                lines.splice(index..end, item);
                changed = true;
                index += 1;
                continue;
            }
        }
        index += 1;
    }

    changed
}

fn migrate_all_branches_log_cmd(lines: &mut Vec<String>, changes: &mut ChangesSet) -> bool {
    let Some(git_idx) = lines.iter().position(|line| line.trim() == "git:") else {
        return false;
    };
    let base_indent = indent_of(&lines[git_idx]);
    let mut cmd_idx = None;
    let mut cmds_idx = None;
    let mut index = git_idx + 1;
    while index < lines.len() {
        let trimmed = lines[index].trim_start();
        let indent = indent_of(&lines[index]);
        if !trimmed.is_empty() && indent <= base_indent {
            break;
        }
        if indent == base_indent + 2 {
            if trimmed.starts_with("allBranchesLogCmd:") {
                cmd_idx = Some(index);
            } else if trimmed.starts_with("allBranchesLogCmds:") {
                cmds_idx = Some(index);
            }
        }
        index += 1;
    }

    let Some(cmd_idx) = cmd_idx else {
        return false;
    };
    let had_cmd = true;
    let cmd_line = lines[cmd_idx].trim_start().to_string();
    let cmd_value = cmd_line
        .strip_prefix("allBranchesLogCmd:")
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let cmd_indent = indent_of(&lines[cmd_idx]);
    let mut changed = false;

    if !cmd_value.is_empty() {
        if let Some(cmds_idx) = cmds_idx {
            let cmds_line = lines[cmds_idx].trim_start().to_string();
            if let Some(inner) = cmds_line
                .strip_prefix("allBranchesLogCmds:")
                .map(str::trim)
                .filter(|value| value.starts_with('[') && value.ends_with(']'))
            {
                let inner = &inner[1..inner.len() - 1];
                let items = if inner.trim().is_empty() {
                    vec![cmd_value.clone()]
                } else {
                    std::iter::once(cmd_value.clone())
                        .chain(inner.split(',').map(str::trim).map(ToOwned::to_owned))
                        .collect::<Vec<_>>()
                };
                lines[cmds_idx] = format!(
                    "{}allBranchesLogCmds: [{}]",
                    " ".repeat(cmd_indent),
                    items.join(", ")
                );
            } else {
                lines.insert(
                    cmds_idx + 1,
                    format!("{}- {}", " ".repeat(cmd_indent + 2), cmd_value),
                );
            }
            changes.add("Prepended git.allBranchesLogCmd value to git.allBranchesLogCmds array");
            changed = true;
        } else {
            lines.splice(
                cmd_idx..=cmd_idx,
                [
                    format!("{}allBranchesLogCmds:", " ".repeat(cmd_indent)),
                    format!("{}- {}", " ".repeat(cmd_indent + 2), cmd_value),
                ],
            );
            changes.add(
                "Created git.allBranchesLogCmds array containing value of git.allBranchesLogCmd",
            );
            changed = true;
        }
    }

    let removal_idx = if changed && cmds_idx.is_none() {
        None
    } else {
        Some(cmd_idx)
    };

    if let Some(removal_idx) = removal_idx {
        lines.remove(removal_idx);
        changed = true;
    } else if changed {
        let obsolete_idx = lines
            .iter()
            .position(|line| line.trim_start().starts_with("allBranchesLogCmd:"));
        if let Some(obsolete_idx) = obsolete_idx {
            lines.remove(obsolete_idx);
        }
    }

    if had_cmd {
        changes.add("Removed obsolete git.allBranchesLogCmd");
        changed = true;
    }

    changed
}

fn migrate_pagers(lines: &mut Vec<String>, changes: &mut ChangesSet) -> bool {
    let Some(git_idx) = lines.iter().position(|line| line.trim() == "git:") else {
        return false;
    };
    let base_indent = indent_of(&lines[git_idx]);
    let mut paging_idx = None;
    let mut pagers_exists = false;
    let mut index = git_idx + 1;
    while index < lines.len() {
        let trimmed = lines[index].trim_start();
        let indent = indent_of(&lines[index]);
        if !trimmed.is_empty() && indent <= base_indent {
            break;
        }
        if indent == base_indent + 2 {
            if trimmed.starts_with("paging:") {
                paging_idx = Some(index);
            } else if trimmed.starts_with("pagers:") {
                pagers_exists = true;
            }
        }
        index += 1;
    }

    if pagers_exists {
        return false;
    }
    let Some(paging_idx) = paging_idx else {
        return false;
    };
    let paging_indent = indent_of(&lines[paging_idx]);
    let trimmed = lines[paging_idx].trim_start().to_string();
    if trimmed == "paging: {}" {
        lines[paging_idx] = format!("{}pagers: [{{}}]", " ".repeat(paging_indent));
        changes.add("Moved git.paging object to git.pagers array");
        return true;
    }
    if trimmed != "paging:" {
        return false;
    }

    let end = next_dedent(lines, paging_idx + 1, paging_indent);
    if paging_idx + 1 >= end {
        return false;
    }
    let mut replacement = vec![format!("{}pagers:", " ".repeat(paging_indent))];
    let mut first = true;
    for line in &lines[(paging_idx + 1)..end] {
        if line.trim().is_empty() {
            replacement.push(line.clone());
            continue;
        }
        let trimmed = line.trim_start();
        let indent = if first {
            paging_indent + 2
        } else {
            paging_indent + 4
        };
        let prefix = if first { "- " } else { "" };
        replacement.push(format!("{}{}{}", " ".repeat(indent), prefix, trimmed));
        first = false;
    }
    lines.splice(paging_idx..end, replacement);
    changes.add("Moved git.paging object to git.pagers array");
    true
}

fn process_custom_command_item(
    item_lines: &[String],
    item_indent: usize,
    changes: &mut ChangesSet,
) -> Option<Vec<String>> {
    let mut subprocess = None;
    let mut stream = None;
    let mut show_output = None;
    let mut command_index = None;
    let mut filtered = Vec::new();

    for line in item_lines {
        let trimmed = line.trim_start();
        if command_index.is_none() && trimmed.starts_with("- command:") {
            command_index = Some(filtered.len());
            filtered.push(line.clone());
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("subprocess:") {
            subprocess = Some(value.trim().to_string());
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("stream:") {
            stream = Some(value.trim().to_string());
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("showOutput:") {
            show_output = Some(value.trim().to_string());
            continue;
        }
        filtered.push(line.clone());
    }

    if subprocess.is_none() && stream.is_none() && show_output.is_none() {
        return None;
    }

    let selected_output = if subprocess.as_deref() == Some("true") {
        Some(("subprocess", "terminal"))
    } else if stream.as_deref() == Some("true") {
        Some(("stream", "log"))
    } else if show_output.as_deref() == Some("true") {
        Some(("showOutput", "popup"))
    } else {
        None
    };

    for (name, value) in [
        ("subprocess", subprocess.as_deref()),
        ("stream", stream.as_deref()),
        ("showOutput", show_output.as_deref()),
    ] {
        let Some(value) = value else {
            continue;
        };
        if let Some((selected_name, output)) = selected_output {
            if name == selected_name && value == "true" {
                changes.add(format!(
                    "Changed '{name}: true' to 'output: {output}' in custom command"
                ));
                continue;
            }
        }
        let suffix = if name == "subprocess" {
            format!("Deleted redundant '{name}: {value}' in custom command")
        } else {
            format!("Deleted redundant '{name}: {value}' property in custom command")
        };
        changes.add(suffix);
    }

    if let Some((_, output)) = selected_output {
        let insert_at = command_index.map_or(0, |index| index + 1);
        filtered.insert(
            insert_at,
            format!("{}output: {output}", " ".repeat(item_indent + 2)),
        );
    }

    while filtered.last().is_some_and(|line| line.trim().is_empty()) {
        filtered.pop();
    }

    Some(filtered)
}

fn convert_mapping_block_to_sequence(lines: &mut Vec<String>, key_idx: usize) -> Option<()> {
    let key_indent = indent_of(&lines[key_idx]);
    let start = key_idx + 1;
    if start >= lines.len() {
        return None;
    }
    let first = lines[start].trim_start();
    if first.is_empty() || first.starts_with("- ") || indent_of(&lines[start]) <= key_indent {
        return None;
    }
    let end = next_dedent(lines, start, key_indent);
    let mut replacement = Vec::new();
    let mut first_line = true;
    for line in &lines[start..end] {
        if line.trim().is_empty() {
            replacement.push(line.clone());
            continue;
        }
        let indent = if first_line {
            key_indent + 2
        } else {
            key_indent + 4
        };
        let prefix = if first_line { "- " } else { "" };
        replacement.push(format!(
            "{}{}{}",
            " ".repeat(indent),
            prefix,
            line.trim_start()
        ));
        first_line = false;
    }
    lines.splice(start..end, replacement);
    Some(())
}

fn next_dedent(lines: &[String], start: usize, parent_indent: usize) -> usize {
    let mut index = start;
    while index < lines.len() {
        let trimmed = lines[index].trim_start();
        if !trimmed.is_empty() && indent_of(&lines[index]) <= parent_indent {
            break;
        }
        index += 1;
    }
    index
}

fn next_sequence_or_dedent(lines: &[String], start: usize, item_indent: usize) -> usize {
    let mut index = start;
    while index < lines.len() {
        let trimmed = lines[index].trim_start();
        let indent = indent_of(&lines[index]);
        if !trimmed.is_empty() && indent < item_indent {
            break;
        }
        if !trimmed.is_empty() && indent == item_indent && trimmed.starts_with("- ") {
            break;
        }
        index += 1;
    }
    index
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_of_renamed_keys_matches_upstream_cases() {
        let scenarios = vec![
            ("", None, false, Vec::<String>::new()),
            (
                "foo:\n  bar: 5\n",
                None,
                false,
                Vec::<String>::new(),
            ),
            (
                "gui:\n  skipUnstageLineWarning: true\n",
                Some("gui:\n  skipDiscardChangeWarning: true\n"),
                true,
                vec!["Renamed 'gui.skipUnstageLineWarning' to 'skipDiscardChangeWarning'".to_string()],
            ),
            (
                "gui:\n  windowSize: half\n  skipUnstageLineWarning: true\nkeybinding:\n  universal:\n    executeCustomCommand: a\n",
                Some("gui:\n  screenMode: half\n  skipDiscardChangeWarning: true\nkeybinding:\n  universal:\n    executeShellCommand: a\n"),
                true,
                vec![
                    "Renamed 'gui.skipUnstageLineWarning' to 'skipDiscardChangeWarning'".to_string(),
                    "Renamed 'keybinding.universal.executeCustomCommand' to 'executeShellCommand'".to_string(),
                    "Renamed 'gui.windowSize' to 'screenMode'".to_string(),
                ],
            ),
        ];

        for (input, expected, expected_changed, expected_changes) in scenarios {
            let mut changes = ChangesSet::new();
            let (actual, did_change) =
                compute_migrated_config("path", input.as_bytes(), &mut changes).expect("migrate");
            assert_eq!(did_change, expected_changed);
            if let Some(expected) = expected {
                assert_eq!(String::from_utf8(actual).expect("utf8"), expected);
            }
            assert_eq!(changes.to_slice_from_oldest(), expected_changes);
        }
    }

    #[test]
    fn null_keybindings_migration_matches_upstream_cases() {
        let scenarios = vec![
            ("", None, false, Vec::<String>::new()),
            (
                "keybinding:\n  universal:\n    quit: q\n",
                None,
                false,
                Vec::<String>::new(),
            ),
            (
                "keybinding:\n  universal:\n    quit: null\n",
                Some("keybinding:\n  universal:\n    quit: <disabled>\n"),
                true,
                vec![
                    "Changed 'null' to '<disabled>' for keybinding 'keybinding.universal.quit'"
                        .to_string(),
                ],
            ),
            (
                "keybinding:\n  universal:\n    quit: null\n    return: <esc>\n    new: null\n",
                Some("keybinding:\n  universal:\n    quit: <disabled>\n    return: <esc>\n    new: <disabled>\n"),
                true,
                vec![
                    "Changed 'null' to '<disabled>' for keybinding 'keybinding.universal.quit'"
                        .to_string(),
                    "Changed 'null' to '<disabled>' for keybinding 'keybinding.universal.new'"
                        .to_string(),
                ],
            ),
        ];

        for (input, expected, expected_changed, expected_changes) in scenarios {
            let mut changes = ChangesSet::new();
            let (actual, did_change) =
                compute_migrated_config("path", input.as_bytes(), &mut changes).expect("migrate");
            assert_eq!(did_change, expected_changed);
            if let Some(expected) = expected {
                assert_eq!(String::from_utf8(actual).expect("utf8"), expected);
            }
            assert_eq!(changes.to_slice_from_oldest(), expected_changes);
        }
    }

    #[test]
    fn commit_prefix_migrations_match_upstream_cases() {
        let scenarios = vec![
            ("", None, false, Vec::<String>::new()),
            (
                "git:\n  commitPrefix:\n     pattern: \"^\\\\w+-\\\\w+.*\"\n     replace: '[JIRA $0] '\n",
                Some("git:\n  commitPrefix:\n    - pattern: \"^\\\\w+-\\\\w+.*\"\n      replace: '[JIRA $0] '\n"),
                true,
                vec!["Changed 'git.commitPrefix' to an array of strings".to_string()],
            ),
            (
                "git:\n  commitPrefixes:\n    foo:\n      pattern: \"^\\\\w+-\\\\w+.*\"\n      replace: '[OTHER $0] '\n    CrazyName!@#$^*&)_-)[[}{f{[]:\n      pattern: \"^foo.bar*\"\n      replace: '[FUN $0] '\n",
                Some("git:\n  commitPrefixes:\n    foo:\n      - pattern: \"^\\\\w+-\\\\w+.*\"\n        replace: '[OTHER $0] '\n    CrazyName!@#$^*&)_-)[[}{f{[]:\n      - pattern: \"^foo.bar*\"\n        replace: '[FUN $0] '\n"),
                true,
                vec!["Changed 'git.commitPrefixes' elements to arrays of strings".to_string()],
            ),
            ("git:", None, false, Vec::<String>::new()),
            (
                "\ngit:\n   commitPrefix:\n    - pattern: \"Hello World\"\n      replace: \"Goodbye\"\n   commitPrefixes:\n    foo:\n      - pattern: \"^\\\\w+-\\\\w+.*\"\n        replace: '[JIRA $0] '",
                None,
                false,
                Vec::<String>::new(),
            ),
        ];

        for (input, expected, expected_changed, expected_changes) in scenarios {
            let mut changes = ChangesSet::new();
            let (actual, did_change) =
                compute_migrated_config("path", input.as_bytes(), &mut changes).expect("migrate");
            assert_eq!(did_change, expected_changed);
            if let Some(expected) = expected {
                assert_eq!(String::from_utf8(actual).expect("utf8"), expected);
            }
            assert_eq!(changes.to_slice_from_oldest(), expected_changes);
        }
    }

    #[test]
    fn custom_command_output_migration_matches_upstream_cases() {
        let scenarios = vec![
            ("", None, false, Vec::<String>::new()),
            (
                "customCommands:\n  - command: echo 'hello'\n    subprocess: true\n  ",
                Some("customCommands:\n  - command: echo 'hello'\n    output: terminal\n"),
                true,
                vec![
                    "Changed 'subprocess: true' to 'output: terminal' in custom command"
                        .to_string(),
                ],
            ),
            (
                "customCommands:\n  - command: echo 'hello'\n    stream: true\n  ",
                Some("customCommands:\n  - command: echo 'hello'\n    output: log\n"),
                true,
                vec!["Changed 'stream: true' to 'output: log' in custom command".to_string()],
            ),
            (
                "customCommands:\n  - command: echo 'hello'\n    showOutput: true\n  ",
                Some("customCommands:\n  - command: echo 'hello'\n    output: popup\n"),
                true,
                vec![
                    "Changed 'showOutput: true' to 'output: popup' in custom command"
                        .to_string(),
                ],
            ),
            (
                "customCommands:\n  - command: echo 'hello'\n    subprocess: true\n    stream: true\n    showOutput: true\n  ",
                Some("customCommands:\n  - command: echo 'hello'\n    output: terminal\n"),
                true,
                vec![
                    "Changed 'subprocess: true' to 'output: terminal' in custom command"
                        .to_string(),
                    "Deleted redundant 'stream: true' property in custom command".to_string(),
                    "Deleted redundant 'showOutput: true' property in custom command".to_string(),
                ],
            ),
            (
                "customCommands:\n  - command: echo 'hello'\n    stream: true\n    showOutput: true\n  ",
                Some("customCommands:\n  - command: echo 'hello'\n    output: log\n"),
                true,
                vec![
                    "Changed 'stream: true' to 'output: log' in custom command".to_string(),
                    "Deleted redundant 'showOutput: true' property in custom command".to_string(),
                ],
            ),
            (
                "customCommands:\n  - command: echo 'hello'\n    subprocess: false\n    stream: false\n    showOutput: false\n  ",
                Some("customCommands:\n  - command: echo 'hello'\n"),
                true,
                vec![
                    "Deleted redundant 'subprocess: false' in custom command".to_string(),
                    "Deleted redundant 'stream: false' property in custom command".to_string(),
                    "Deleted redundant 'showOutput: false' property in custom command".to_string(),
                ],
            ),
        ];

        for (input, expected, expected_changed, expected_changes) in scenarios {
            let mut changes = ChangesSet::new();
            let (actual, did_change) =
                compute_migrated_config("path", input.as_bytes(), &mut changes).expect("migrate");
            assert_eq!(did_change, expected_changed);
            if let Some(expected) = expected {
                assert_eq!(String::from_utf8(actual).expect("utf8"), expected);
            }
            assert_eq!(changes.to_slice_from_oldest(), expected_changes);
        }
    }

    #[test]
    fn all_branches_log_cmd_migration_matches_upstream_cases() {
        let scenarios = vec![
            ("git:", None, false, Vec::<String>::new()),
            (
                "git:\n  allBranchesLogCmd: git log --graph --oneline\n",
                Some("git:\n  allBranchesLogCmds:\n    - git log --graph --oneline\n"),
                true,
                vec![
                    "Created git.allBranchesLogCmds array containing value of git.allBranchesLogCmd".to_string(),
                    "Removed obsolete git.allBranchesLogCmd".to_string(),
                ],
            ),
            (
                "git:\n  allBranchesLogCmd: git log --graph --oneline\n  allBranchesLogCmds:\n    - git log --graph --oneline --pretty\n",
                Some("git:\n  allBranchesLogCmds:\n    - git log --graph --oneline\n    - git log --graph --oneline --pretty\n"),
                true,
                vec![
                    "Prepended git.allBranchesLogCmd value to git.allBranchesLogCmds array".to_string(),
                    "Removed obsolete git.allBranchesLogCmd".to_string(),
                ],
            ),
            (
                "git:\n  allBranchesLogCmds:\n    - git log\n",
                None,
                false,
                Vec::<String>::new(),
            ),
            (
                "git:\n  allBranchesLogCmds:\n    - git log --graph --oneline\n  allBranchesLogCmd:\n",
                Some("git:\n  allBranchesLogCmds:\n    - git log --graph --oneline\n"),
                true,
                vec!["Removed obsolete git.allBranchesLogCmd".to_string()],
            ),
            (
                "git:\n  allBranchesLogCmds: [foo, bar]\n  allBranchesLogCmd: baz\n",
                Some("git:\n  allBranchesLogCmds: [baz, foo, bar]\n"),
                true,
                vec![
                    "Prepended git.allBranchesLogCmd value to git.allBranchesLogCmds array".to_string(),
                    "Removed obsolete git.allBranchesLogCmd".to_string(),
                ],
            ),
            (
                "git:\n  allBranchesLogCmds:\n    - git log --graph --oneline\n  allBranchesLogCmd:\n  foo: bar\n",
                Some("git:\n  allBranchesLogCmds:\n    - git log --graph --oneline\n  foo: bar\n"),
                true,
                vec!["Removed obsolete git.allBranchesLogCmd".to_string()],
            ),
        ];

        for (input, expected, expected_changed, expected_changes) in scenarios {
            let mut changes = ChangesSet::new();
            let (actual, did_change) =
                compute_migrated_config("path", input.as_bytes(), &mut changes).expect("migrate");
            assert_eq!(did_change, expected_changed);
            if let Some(expected) = expected {
                assert_eq!(String::from_utf8(actual).expect("utf8"), expected);
            }
            assert_eq!(changes.to_slice_from_oldest(), expected_changes);
        }
    }

    #[test]
    fn pager_migration_matches_upstream_cases() {
        let scenarios = vec![
            ("git:", None, false, Vec::<String>::new()),
            (
                "git:\n  autoFetch: true\n",
                None,
                false,
                Vec::<String>::new(),
            ),
            (
                "git:\n  paging:\n    pager: delta --dark --paging=never\n  pagers:\n    - diff: diff-so-fancy\n",
                None,
                false,
                Vec::<String>::new(),
            ),
            (
                "git:\n  paging: 5\n",
                None,
                false,
                Vec::<String>::new(),
            ),
            (
                "git:\n  paging:\n    pager: delta --dark --paging=never\n  autoFetch: true\n",
                Some("git:\n  pagers:\n    - pager: delta --dark --paging=never\n  autoFetch: true\n"),
                true,
                vec!["Moved git.paging object to git.pagers array".to_string()],
            ),
            (
                "git:\n  paging: {}\n",
                Some("git:\n  pagers: [{}]\n"),
                true,
                vec!["Moved git.paging object to git.pagers array".to_string()],
            ),
        ];

        for (input, expected, expected_changed, expected_changes) in scenarios {
            let mut changes = ChangesSet::new();
            let (actual, did_change) =
                compute_migrated_config("path", input.as_bytes(), &mut changes).expect("migrate");
            assert_eq!(did_change, expected_changed);
            if let Some(expected) = expected {
                assert_eq!(String::from_utf8(actual).expect("utf8"), expected);
            }
            assert_eq!(changes.to_slice_from_oldest(), expected_changes);
        }
    }
}
