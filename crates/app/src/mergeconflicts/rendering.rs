// Ported from ./references/lazygit-master/pkg/gui/mergeconflicts/rendering.go

use crate::mergeconflicts::merge_conflict::MergeConflict;

pub fn colored_conflict_file(content: &str, conflicts: &[MergeConflict]) -> String {
    if conflicts.is_empty() {
        return content.to_string();
    }

    let mut conflict = conflicts[0];
    let remaining = &conflicts[1..];

    let mut result = String::new();
    for (i, line) in content.lines().enumerate() {
        let text_style = if conflict.is_marker_line(i as i32) {
            "RED"
        } else {
            "DEFAULT"
        };

        if i as i32 == conflict.end && !remaining.is_empty() {
            conflict = remaining[0];
        }

        result.push_str(&format!("{}[{}]\n", text_style, line));
    }
    result
}

pub fn shift_conflict(conflicts: &[MergeConflict]) -> (&MergeConflict, &[MergeConflict]) {
    (&conflicts[0], &conflicts[1..])
}
