// Ported from ./references/lazygit-master/pkg/gui/filetree/node.go

use std::path;

use super::collapsed_paths::CollapsedPaths;

pub struct Node<T> {
    pub file: Option<T>,
    pub children: Vec<Node<T>>,
    path: String,
    compression_level: i32,
}

impl<T> Node<T> {
    pub fn new(path: String, file: Option<T>) -> Self {
        Self {
            file,
            children: Vec::new(),
            path,
            compression_level: 0,
        }
    }

    pub fn is_file(&self) -> bool {
        self.file.is_some()
    }

    pub fn get_file(&self) -> Option<&T> {
        self.file.as_ref()
    }

    pub fn get_path(&self) -> String {
        self.path
            .strip_prefix("./")
            .unwrap_or(&self.path)
            .to_string()
    }

    pub fn get_internal_path(&self) -> &str {
        &self.path
    }

    pub fn sort(&mut self)
    where
        T: Clone,
    {
        self.sort_children();
        for child in &mut self.children {
            child.sort();
        }
    }

    pub fn for_each_file<F, E>(&self, mut cb: F) -> Result<(), E>
    where
        F: FnMut(&T) -> Result<(), E>,
    {
        if self.is_file() {
            if let Some(ref file) = self.file {
                cb(file)?;
            }
        }

        for child in &self.children {
            child.for_each_file(&mut cb)?;
        }

        Ok(())
    }

    fn sort_children(&mut self)
    where
        T: Clone,
    {
        if self.is_file() {
            return;
        }

        let mut children = self.children.to_vec();

        children.sort_by(|a, b| {
            let a_is_file = a.is_file();
            let b_is_file = b.is_file();
            if !a_is_file && b_is_file {
                return std::cmp::Ordering::Less;
            }
            if a_is_file && !b_is_file {
                return std::cmp::Ordering::Greater;
            }
            a.path.cmp(&b.path)
        });

        self.children = children;
    }

    pub fn some(&self, predicate: impl Fn(&Node<T>) -> bool) -> bool {
        if predicate(self) {
            return true;
        }

        for child in &self.children {
            if child.some(&predicate) {
                return true;
            }
        }

        false
    }

    pub fn some_file(&self, predicate: impl Fn(&T) -> bool) -> bool {
        if self.is_file() {
            if let Some(ref file) = self.file {
                if predicate(file) {
                    return true;
                }
            }
        } else {
            for child in &self.children {
                if child.some_file(&predicate) {
                    return true;
                }
            }
        }

        false
    }

    pub fn every(&self, predicate: impl Fn(&Node<T>) -> bool) -> bool {
        if !predicate(self) {
            return false;
        }

        for child in &self.children {
            if !child.every(&predicate) {
                return false;
            }
        }

        true
    }

    pub fn every_file(&self, predicate: impl Fn(&T) -> bool) -> bool {
        if self.is_file() {
            if let Some(ref file) = self.file {
                if !predicate(file) {
                    return false;
                }
            }
        } else {
            for child in &self.children {
                if !child.every_file(&predicate) {
                    return false;
                }
            }
        }

        true
    }

    pub fn find_first_file_by(&self, predicate: impl Fn(&T) -> bool) -> Option<&T> {
        if self.is_file() {
            if let Some(ref file) = self.file {
                if predicate(file) {
                    return Some(file);
                }
            }
        } else {
            for child in &self.children {
                if let Some(file) = child.find_first_file_by(&predicate) {
                    return Some(file);
                }
            }
        }

        None
    }

    pub fn flatten(&self, collapsed_paths: &CollapsedPaths) -> Vec<&Node<T>> {
        let mut result = vec![self];

        if !self.children.is_empty() && !collapsed_paths.is_collapsed(&self.path) {
            let mut flattened_children: Vec<&Node<T>> = self
                .children
                .iter()
                .flat_map(|child| child.flatten(collapsed_paths))
                .collect();
            result.append(&mut flattened_children);
        }

        result
    }

    pub fn get_node_at_index(
        &self,
        index: usize,
        collapsed_paths: &CollapsedPaths,
    ) -> Option<&Node<T>> {
        if self.is_file() {
            return None;
        }

        let (node, _, _) = self.get_node_at_index_aux(index, collapsed_paths, -1);

        node
    }

    pub fn get_visual_depth_at_index(&self, index: usize, collapsed_paths: &CollapsedPaths) -> i32 {
        if self.is_file() {
            return -1;
        }

        let (_, _, depth) = self.get_node_at_index_aux(index, collapsed_paths, -1);

        depth
    }

    fn get_node_at_index_aux(
        &self,
        index: usize,
        collapsed_paths: &CollapsedPaths,
        visual_depth: i32,
    ) -> (Option<&Node<T>>, usize, i32) {
        let offset = 1;

        if index == 0 {
            return (Some(self), offset, visual_depth);
        }

        if !collapsed_paths.is_collapsed(&self.path) {
            let mut current_offset = offset;
            for child in &self.children {
                let (found_node, offset_change, depth) = child.get_node_at_index_aux(
                    index - current_offset,
                    collapsed_paths,
                    visual_depth + 1,
                );
                current_offset += offset_change;
                if found_node.is_some() {
                    return (found_node, current_offset, depth);
                }
            }
        }

        (None, offset, -1)
    }

    pub fn get_index_for_path(
        &self,
        path: &str,
        collapsed_paths: &CollapsedPaths,
    ) -> (usize, bool) {
        let mut offset = 0;

        if self.path == path {
            return (offset, true);
        }

        if !collapsed_paths.is_collapsed(&self.path) {
            for child in &self.children {
                let (offset_change, found) = child.get_index_for_path(path, collapsed_paths);
                offset += offset_change + 1;
                if found {
                    return (offset, true);
                }
            }
        }

        (offset, false)
    }

    pub fn size(&self, collapsed_paths: &CollapsedPaths) -> usize {
        if self.is_file() {
            return 1;
        }

        let mut output = 1;

        if !collapsed_paths.is_collapsed(&self.path) {
            for child in &self.children {
                output += child.size(collapsed_paths);
            }
        }

        output
    }

    pub fn compress(&mut self)
    where
        T: Clone,
    {
        if self.is_file() {
            return;
        }

        self.compress_aux();
    }

    fn compress_aux(&mut self) -> &mut Node<T>
    where
        T: Clone,
    {
        if self.is_file() {
            return self;
        }

        let mut i = 0;
        while i < self.children.len() {
            let child = &mut self.children[i];
            while child.children.len() == 1 && !child.children[0].is_file() {
                child.children[0].compression_level = child.compression_level + 1;
                let compressed = child.children[0].clone();
                child.children = compressed.children;
                if child.children.is_empty() {
                    break;
                }
            }
            i += 1;
        }

        for child in &mut self.children {
            child.compress_aux();
        }

        self
    }

    pub fn get_paths_matching(&self, predicate: impl Fn(&Node<T>) -> bool) -> Vec<String> {
        let mut paths = Vec::new();

        if predicate(self) {
            paths.push(self.get_path());
        }

        for child in &self.children {
            paths.append(&mut child.get_paths_matching(&predicate));
        }

        paths
    }

    pub fn get_file_paths_matching(&self, predicate: impl Fn(&T) -> bool) -> Vec<String> {
        self.get_leaves()
            .into_iter()
            .filter_map(|node| {
                if let Some(ref file) = node.file {
                    if predicate(file) {
                        return Some(node.get_path());
                    }
                }
                None
            })
            .collect()
    }

    pub fn get_leaves(&self) -> Vec<&Node<T>> {
        if self.is_file() {
            return vec![self];
        }

        self.children
            .iter()
            .flat_map(|child| child.get_leaves())
            .collect()
    }

    pub fn id(&self) -> String {
        self.get_path()
    }

    pub fn description(&self) -> String {
        self.get_path()
    }

    pub fn name(&self) -> String {
        path::Path::new(&self.path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }
}

impl<T: Clone> Clone for Node<T> {
    fn clone(&self) -> Self {
        Self {
            file: self.file.clone(),
            children: self.children.clone(),
            path: self.path.clone(),
            compression_level: self.compression_level,
        }
    }
}
