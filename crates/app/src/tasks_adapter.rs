// Ported from ./references/lazygit-master/pkg/gui/tasks_adapter.go

use std::collections::HashMap;
use std::process::Command;

pub struct TasksAdapter;

impl TasksAdapter {
    pub fn new() -> Self {
        Self
    }

    pub fn new_cmd_task(
        &self,
        _view_name: &str,
        _cmd: &mut Command,
        _prefix: &str,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn new_string_task(&self, _view_name: &str, _content: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn new_string_task_without_scroll(
        &self,
        _view_name: &str,
        _content: &str,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn new_string_task_with_scroll(
        &self,
        _view_name: &str,
        _content: &str,
        _origin_x: i32,
        _origin_y: i32,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn new_string_task_with_key(
        &self,
        _view_name: &str,
        _content: &str,
        _key: &str,
    ) -> Result<(), String> {
        Ok(())
    }
}

impl Default for TasksAdapter {
    fn default() -> Self {
        Self::new()
    }
}
