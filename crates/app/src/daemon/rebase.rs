use std::path::Path;

use crate::utils::rebase_todo::{
    self, drop_merge_commit, edit_rebase_todo, move_fixup_commit_down,
    move_todos_down, move_todos_up, prepend_str_to_todo_file,
    read_rebase_todo_file, remove_update_refs_for_copied_branch, write_rebase_todo_file,
    Todo, TodoChange,
};

pub fn handle_interactive_rebase<F>(path: &Path, f: F) -> anyhow::Result<()>
where
    F: FnOnce(&Path) -> anyhow::Result<()>,
{
    let path_str = path.to_string_lossy();
    let comment_char = get_comment_char();

    if path_str.ends_with("git-rebase-todo") {
        let path_str = path.to_string_lossy();
        remove_update_refs_for_copied_branch(&path_str, comment_char)?;
        f(path)?;
    } else if path_str.ends_with("COMMIT_EDITMSG") {
    }
    Ok(())
}

pub fn edit_todo_actions(path: &Path, changes: Vec<TodoChange>) -> anyhow::Result<()> {
    let comment_char = get_comment_char();
    let path_str = path.to_string_lossy();
    edit_rebase_todo(&path_str, &changes, comment_char)
}

pub fn drop_merge(path: &Path, hash: &str) -> anyhow::Result<()> {
    let comment_char = get_comment_char();
    let path_str = path.to_string_lossy();
    drop_merge_commit(&path_str, hash, comment_char)
}

pub fn move_fixup_down(
    path: &Path,
    original_hash: &str,
    fixup_hash: &str,
    change_to_fixup: bool,
) -> anyhow::Result<()> {
    let comment_char = get_comment_char();
    let path_str = path.to_string_lossy();
    move_fixup_commit_down(&path_str, original_hash, fixup_hash, change_to_fixup, comment_char)
}

pub fn move_up(path: &Path, hashes: &[String]) -> anyhow::Result<()> {
    let comment_char = get_comment_char();
    let path_str = path.to_string_lossy();
    let todos_to_move: Vec<Todo> = hashes
        .iter()
        .map(|h| Todo {
            hash: h.clone(),
            r#ref: String::new(),
        })
        .collect();
    let todos = read_rebase_todo_file(&path_str, comment_char)?;
    let rearranged = move_todos_up(todos, &todos_to_move, false);
    write_rebase_todo_file(&path_str, &rearranged, comment_char)
}

pub fn move_down(path: &Path, hashes: &[String]) -> anyhow::Result<()> {
    let comment_char = get_comment_char();
    let path_str = path.to_string_lossy();
    let todos_to_move: Vec<Todo> = hashes
        .iter()
        .map(|h| Todo {
            hash: h.clone(),
            r#ref: String::new(),
        })
        .collect();
    let todos = read_rebase_todo_file(&path_str, comment_char)?;
    let rearranged = move_todos_down(todos, &todos_to_move, false);
    write_rebase_todo_file(&path_str, &rearranged, comment_char)
}

pub fn insert_break(path: &Path) -> anyhow::Result<()> {
    let path_str = path.to_string_lossy();
    prepend_str_to_todo_file(&path_str, b"break\n")
}

pub fn write_todo_content(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    std::fs::write(path, content)?;
    Ok(())
}

pub fn remove_update_refs(path: &Path) -> anyhow::Result<()> {
    let comment_char = get_comment_char();
    let path_str = path.to_string_lossy();
    remove_update_refs_for_copied_branch(&path_str, comment_char)
}

fn get_comment_char() -> char {
    let output = std::process::Command::new("git")
        .args(["config", "--get", "--null", "core.commentChar"])
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.len() == 2 {
            return stdout.chars().next().unwrap_or('#');
        }
    }

    '#'
}
    // Other paths are silently ignored

    Ok(())
}

/// Edit rebase todo file with changes
pub fn edit_todo_actions(path: &Path, changes: Vec<TodoChange>) -> anyhow::Result<()> {
    let comment_char = get_comment_char();
    edit_rebase_todo(path, changes, comment_char)
}

/// Drop a merge commit from the rebase todo
pub fn drop_merge(path: &Path, hash: &str) -> anyhow::Result<()> {
    let comment_char = get_comment_char();
    drop_merge_commit(path, hash, comment_char)
}

/// Move a fixup commit down to right after the original commit
pub fn move_fixup_down(
    path: &Path,
    original_hash: &str,
    fixup_hash: &str,
    change_to_fixup: bool,
) -> anyhow::Result<()> {
    let comment_char = get_comment_char();
    move_fixup_commit_down(
        path,
        original_hash,
        fixup_hash,
        change_to_fixup,
        comment_char,
    )
}

/// Move todos up in the rebase todo list
pub fn move_up(path: &Path, hashes: &[String]) -> anyhow::Result<()> {
    let comment_char = get_comment_char();
    let todos_to_move: Vec<Todo> = hashes
        .iter()
        .map(|h| Todo {
            hash: h.clone(),
            r#ref: String::new(),
        })
        .collect();
    let todos = read_rebase_todo_file(path.to_str().unwrap_or(""), comment_char)?;
    let rearranged = move_todos_up(todos, &todos_to_move, false);
    write_rebase_todo_file(path.to_str().unwrap_or(""), &rearranged, comment_char)
}

/// Move todos down in the rebase todo list
pub fn move_down(path: &Path, hashes: &[String]) -> anyhow::Result<()> {
    let comment_char = get_comment_char();
    let todos_to_move: Vec<Todo> = hashes
        .iter()
        .map(|h| Todo {
            hash: h.clone(),
            r#ref: String::new(),
        })
        .collect();
    let todos = read_rebase_todo_file(path.to_str().unwrap_or(""), comment_char)?;
    let rearranged = move_todos_down(todos, &todos_to_move, false);
    write_rebase_todo_file(path.to_str().unwrap_or(""), &rearranged, comment_char)
}

/// Insert a break at the beginning of the todo file
pub fn insert_break(path: &Path) -> anyhow::Result<()> {
    prepend_str_to_todo_file(path.to_str().unwrap_or(""), b"break\n")
}

/// Write content to the rebase todo file
pub fn write_todo_content(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    std::fs::write(path, content)?;
    Ok(())
}

/// Remove update refs for copied branch
pub fn remove_update_refs(path: &Path) -> anyhow::Result<()> {
    let comment_char = get_comment_char();
    remove_update_refs_for_copied_branch(path, comment_char)
}

fn get_comment_char() -> char {
    let output = std::process::Command::new("git")
        .args(["config", "--get", "--null", "core.commentChar"])
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.len() == 2 {
            return stdout.chars().next().unwrap_or('#');
        }
    }

    '#'
}
