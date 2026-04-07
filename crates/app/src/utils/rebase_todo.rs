use std::fs;
use std::io::{BufRead, BufReader, Write};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Todo {
    pub hash: String,
    pub r#ref: String,
}

#[derive(Debug, Clone)]
pub struct TodoChange {
    pub hash: String,
    pub new_action: TodoCommand,
    pub flag: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoCommand {
    Pick,
    Fixup,
    Squash,
    Reword,
    Edit,
    Drop,
    Merge,
    UpdateRef,
    Reset,
    Label,
    Comment,
    Exec,
}

pub fn equal_hash(a: &str, b: &str) -> bool {
    if a.is_empty() && b.is_empty() {
        return true;
    }

    let common_length = a.len().min(b.len());
    if common_length == 0 {
        return false;
    }

    a[..common_length] == b[..common_length]
}

pub fn read_rebase_todo_file(file_name: &str, comment_char: char) -> std::io::Result<Vec<Todo>> {
    let file = fs::File::open(file_name)?;
    let reader = BufReader::new(file);
    let mut todos = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.starts_with(comment_char) || line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let hash = if parts.len() > 1 {
            parts[1].to_string()
        } else {
            String::new()
        };

        todos.push(Todo {
            hash,
            r#ref: String::new(),
        });
    }

    Ok(todos)
}

pub fn write_rebase_todo_file(
    file_name: &str,
    todos: &[Todo],
    comment_char: char,
) -> std::io::Result<()> {
    let mut file = fs::File::create(file_name)?;
    for todo in todos {
        writeln!(file, "pick {} {}", comment_char, todo.hash)?;
    }
    Ok(())
}

pub fn prepend_str_to_todo_file(file_path: &str, lines_to_prepend: &[u8]) -> std::io::Result<()> {
    let existing_content = fs::read(file_path)?;
    let mut combined = lines_to_prepend.to_vec();
    combined.extend_from_slice(&existing_content);
    fs::write(file_path, combined)
}

pub fn remove_element<T>(vec: Vec<T>, index: usize) -> Vec<T> {
    let mut result = vec;
    if index < result.len() {
        result.remove(index);
    }
    result
}

pub fn move_element<T: Clone>(vec: Vec<T>, from: usize, to: usize) -> Vec<T> {
    if from >= vec.len() || to >= vec.len() || from == to {
        return vec;
    }

    let mut result = vec.clone();
    let element = result.remove(from);
    result.insert(to, element);
    result
}

pub fn find_todo(todos: &[Todo], todo_to_find: &Todo) -> Option<usize> {
    for (i, todo) in todos.iter().enumerate() {
        if equal_hash(&todo.hash, &todo_to_find.hash) && todo.r#ref == todo_to_find.r#ref {
            return Some(i);
        }
    }
    None
}

pub fn delete_todos(todos: &mut Vec<Todo>, todos_to_delete: &[Todo]) -> std::io::Result<()> {
    for todo_to_delete in todos_to_delete {
        if let Some(idx) = find_todo(todos, todo_to_delete) {
            todos.remove(idx);
        }
    }
    Ok(())
}

pub fn move_todo_down(todos: Vec<Todo>, todo_to_move: &Todo, _is_in_rebase: bool) -> Vec<Todo> {
    if let Some(source_idx) = find_todo(&todos, todo_to_move) {
        if source_idx < todos.len() - 1 {
            let mut result = todos.clone();
            let element = result.remove(source_idx);
            result.insert(source_idx + 1, element);
            return result;
        }
    }
    todos
}

pub fn move_todo_up(todos: Vec<Todo>, todo_to_move: &Todo, _is_in_rebase: bool) -> Vec<Todo> {
    if let Some(source_idx) = find_todo(&todos, todo_to_move) {
        if source_idx > 0 {
            let mut result = todos.clone();
            let element = result.remove(source_idx);
            result.insert(source_idx - 1, element);
            return result;
        }
    }
    todos
}

pub fn move_todos_down(todos: Vec<Todo>, todos_to_move: &[Todo], is_in_rebase: bool) -> Vec<Todo> {
    let mut result = todos;
    for todo_to_move in todos_to_move.iter().rev() {
        result = move_todo_down(result, todo_to_move, is_in_rebase);
    }
    result
}

pub fn move_todos_up(todos: Vec<Todo>, todos_to_move: &[Todo], is_in_rebase: bool) -> Vec<Todo> {
    let mut result = todos;
    for todo_to_move in todos_to_move {
        result = move_todo_up(result, todo_to_move, is_in_rebase);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equal_hash_both_empty() {
        assert!(equal_hash("", ""));
    }

    #[test]
    fn test_equal_hash_exact_match() {
        assert!(equal_hash("abc123", "abc123"));
    }

    #[test]
    fn test_equal_hash_prefix_match() {
        assert!(equal_hash("abc123", "abc"));
        assert!(equal_hash("abc", "abc123"));
    }

    #[test]
    fn test_equal_hash_no_match() {
        assert!(!equal_hash("abc", "def"));
    }

    #[test]
    fn test_remove_element() {
        let v = vec![1, 2, 3, 4, 5];
        let result = remove_element(v, 2);
        assert_eq!(result, vec![1, 2, 4, 5]);
    }

    #[test]
    fn test_remove_element_out_of_bounds() {
        let v = vec![1, 2, 3];
        let result = remove_element(v, 10);
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_move_element_forward() {
        let v = vec![1, 2, 3, 4, 5];
        let result = move_element(v, 1, 3);
        assert_eq!(result, vec![1, 3, 4, 2, 5]);
    }

    #[test]
    fn test_move_element_backward() {
        let v = vec![1, 2, 3, 4, 5];
        let result = move_element(v, 3, 1);
        assert_eq!(result, vec![1, 4, 2, 3, 5]);
    }

    #[test]
    fn test_move_element_same_position() {
        let v = vec![1, 2, 3, 4, 5];
        let result = move_element(v, 2, 2);
        assert_eq!(result, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_find_todo() {
        let todos = vec![
            Todo {
                hash: "abc".to_string(),
                r#ref: "".to_string(),
            },
            Todo {
                hash: "def".to_string(),
                r#ref: "".to_string(),
            },
            Todo {
                hash: "ghi".to_string(),
                r#ref: "".to_string(),
            },
        ];
        let to_find = Todo {
            hash: "def".to_string(),
            r#ref: "".to_string(),
        };
        assert_eq!(find_todo(&todos, &to_find), Some(1));
    }

    #[test]
    fn test_find_todo_not_found() {
        let todos = vec![
            Todo {
                hash: "abc".to_string(),
                r#ref: "".to_string(),
            },
            Todo {
                hash: "def".to_string(),
                r#ref: "".to_string(),
            },
        ];
        let to_find = Todo {
            hash: "xyz".to_string(),
            r#ref: "".to_string(),
        };
        assert_eq!(find_todo(&todos, &to_find), None);
    }

    #[test]
    fn test_move_todo_down() {
        let todos = vec![
            Todo {
                hash: "a".to_string(),
                r#ref: "".to_string(),
            },
            Todo {
                hash: "b".to_string(),
                r#ref: "".to_string(),
            },
            Todo {
                hash: "c".to_string(),
                r#ref: "".to_string(),
            },
        ];
        let to_move = Todo {
            hash: "a".to_string(),
            r#ref: "".to_string(),
        };
        let result = move_todo_down(todos, &to_move, false);
        assert_eq!(result[0].hash, "b");
        assert_eq!(result[1].hash, "a");
    }

    #[test]
    fn test_move_todo_up() {
        let todos = vec![
            Todo {
                hash: "a".to_string(),
                r#ref: "".to_string(),
            },
            Todo {
                hash: "b".to_string(),
                r#ref: "".to_string(),
            },
            Todo {
                hash: "c".to_string(),
                r#ref: "".to_string(),
            },
        ];
        let to_move = Todo {
            hash: "c".to_string(),
            r#ref: "".to_string(),
        };
        let result = move_todo_up(todos, &to_move, false);
        assert_eq!(result[1].hash, "c");
        assert_eq!(result[2].hash, "b");
    }
}
