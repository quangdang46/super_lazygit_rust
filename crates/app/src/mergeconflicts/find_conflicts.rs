use std::fs::File;
use std::io::{self, BufRead, BufReader};

use crate::mergeconflicts::merge_conflict::MergeConflict;
use crate::utils::lines::split_lines;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineType {
    Start,
    Ancestor,
    Target,
    End,
    NotAMarker,
}

const CONFLICT_START: &str = "<<<<<<< ";
const CONFLICT_END: &str = ">>>>>>> ";
const CONFLICT_START_BYTES: &[u8] = CONFLICT_START.as_bytes();
const CONFLICT_END_BYTES: &[u8] = CONFLICT_END.as_bytes();

pub fn find_conflicts(content: &str) -> Vec<MergeConflict> {
    let mut conflicts = Vec::new();

    if content.is_empty() {
        return conflicts;
    }

    let mut new_conflict: Option<MergeConflict> = None;
    for (i, line) in split_lines(content).iter().enumerate() {
        match determine_line_type(line) {
            LineType::Start => {
                new_conflict = Some(MergeConflict {
                    start: i as i32,
                    ancestor: -1,
                    target: -1,
                    end: -1,
                });
            }
            LineType::Ancestor => {
                if let Some(ref mut c) = new_conflict {
                    c.ancestor = i as i32;
                }
            }
            LineType::Target => {
                if let Some(ref mut c) = new_conflict {
                    c.target = i as i32;
                }
            }
            LineType::End => {
                if let Some(mut c) = new_conflict.take() {
                    c.end = i as i32;
                    conflicts.push(c);
                }
            }
            LineType::NotAMarker => {}
        }
    }

    conflicts
}

fn determine_line_type(line: &str) -> LineType {
    let trimmed_line = line.strip_prefix("++").unwrap_or(line);

    if trimmed_line.starts_with(CONFLICT_START) {
        LineType::Start
    } else if trimmed_line.starts_with("||||||| ") {
        LineType::Ancestor
    } else if trimmed_line == "=======" {
        LineType::Target
    } else if trimmed_line.starts_with(CONFLICT_END) {
        LineType::End
    } else {
        LineType::NotAMarker
    }
}

pub fn file_has_conflict_markers(path: &str) -> Result<bool, io::Error> {
    let file = File::open(path)?;
    Ok(file_has_conflict_markers_aux(file))
}

fn file_has_conflict_markers_aux(file: File) -> bool {
    let reader = BufReader::new(file);

    for line_result in reader.split(b'\n') {
        let line = match line_result {
            Ok(line) => line,
            Err(_) => continue,
        };

        if line.starts_with(CONFLICT_START_BYTES) || line.starts_with(CONFLICT_END_BYTES) {
            return true;
        }
    }

    false
}
