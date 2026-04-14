// Ported from ./references/lazygit-master/pkg/gui/filetree/build_tree.go

use std::cmp::Ordering;

use crate::filetree::file_filter::{CommitFile, File};
use crate::filetree::node::Node;

pub fn build_tree_from_files(files: &[File], show_root_item: bool) -> Node<File> {
    let mut root = Node::new(String::new(), None);

    for file in files {
        let split_path = split_file_tree_path(&file.path, show_root_item);
        insert_into_tree(&mut root, &split_path, file);
    }

    root.sort();
    root.compress();

    root
}

fn insert_into_tree(node: &mut Node<File>, split_path: &[String], file: &File) {
    let path = join(split_path);
    let is_file = split_path.len() == 1;

    for child in &mut node.children {
        if child.get_internal_path() == path {
            if is_file {
                child.file = Some(file.clone());
            }
            return;
        }
    }

    if node.children.is_empty()
        && node.get_internal_path().is_empty()
        && split_path.len() == 2
        && file.path == split_path[1]
    {
        return;
    }

    let new_child = Node::new(
        path.clone(),
        if is_file { Some(file.clone()) } else { None },
    );
    node.children.push(new_child);
}

pub fn build_flat_tree_from_commit_files(
    files: &[CommitFile],
    show_root_item: bool,
) -> Node<CommitFile> {
    let root_aux = build_tree_from_commit_files(files, show_root_item);
    let sorted_files = root_aux.get_leaves();

    Node::new(String::new(), None)
}

pub fn build_tree_from_commit_files(
    files: &[CommitFile],
    show_root_item: bool,
) -> Node<CommitFile> {
    let mut root = Node::new(String::new(), None);

    for file in files {
        let split_path = split_file_tree_path(&file.path, show_root_item);
        insert_commit_into_tree(&mut root, &split_path, file);
    }

    root.sort();
    root.compress();

    root
}

fn insert_commit_into_tree(node: &mut Node<CommitFile>, split_path: &[String], file: &CommitFile) {
    let path = join(split_path);
    let is_file = split_path.len() == 1;

    for child in &mut node.children {
        if child.get_internal_path() == path {
            if is_file {
                child.file = Some(file.clone());
            }
            return;
        }
    }

    if node.children.is_empty()
        && node.get_internal_path().is_empty()
        && split_path.len() == 2
        && file.path == split_path[1]
    {
        return;
    }

    let new_child = Node::new(
        path.clone(),
        if is_file { Some(file.clone()) } else { None },
    );
    node.children.push(new_child);
}

pub fn build_flat_tree_from_files(files: &[File], show_root_item: bool) -> Node<File> {
    let root_aux = build_tree_from_files(files, show_root_item);
    let mut sorted_files: Vec<&Node<File>> = root_aux.get_leaves();

    sorted_files.sort_by(|a, b| {
        let a_file = if let Some(f) = a.get_file() {
            f
        } else {
            return Ordering::Less;
        };
        let b_file = if let Some(f) = b.get_file() {
            f
        } else {
            return Ordering::Greater;
        };

        if a_file.has_merge_conflicts && !b_file.has_merge_conflicts {
            return Ordering::Less;
        }

        if b_file.has_merge_conflicts && !a_file.has_merge_conflicts {
            return Ordering::Greater;
        }

        if a_file.tracked && !b_file.tracked {
            return Ordering::Less;
        }

        if b_file.tracked && !a_file.tracked {
            return Ordering::Greater;
        }

        Ordering::Equal
    });

    let mut result = Node::new(String::new(), None);
    for file in sorted_files {
        result.children.push((*file).clone());
    }

    result
}

fn split(str: &str) -> Vec<String> {
    str.split('/').map(|s| s.to_string()).collect()
}

fn join(strs: &[String]) -> String {
    strs.join("/")
}

pub fn split_file_tree_path(path: &str, show_root_item: bool) -> Vec<String> {
    split(&internal_tree_path_for_file_path(path, show_root_item))
}

fn internal_tree_path_for_file_path(path: &str, show_root_item: bool) -> String {
    if show_root_item {
        format!("./{}", path)
    } else {
        path.to_string()
    }
}
