use std::collections::HashMap;

pub struct BranchesController {
    context: String,
}

impl BranchesController {
    pub fn new() -> Self {
        Self {
            context: "Branches".to_string(),
        }
    }

    pub fn get_keybindings(&self) -> Vec<Binding> {
        Vec::new()
    }

    pub fn context(&self) -> &str {
        &self.context
    }

    pub fn press(&self, _branch: &Branch) -> Result<(), String> {
        Ok(())
    }

    pub fn delete(&self, _branches: &[Branch]) -> Result<(), String> {
        Ok(())
    }

    pub fn merge(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn rebase(&self, _branch: &Branch) -> Result<(), String> {
        Ok(())
    }

    pub fn fast_forward(&self, _branch: &Branch) -> Result<(), String> {
        Ok(())
    }

    pub fn rename(&self, _branch: &Branch) -> Result<(), String> {
        Ok(())
    }

    pub fn create_tag(&self, _branch: &Branch) -> Result<(), String> {
        Ok(())
    }

    pub fn new_branch(&self, _branch: &Branch) -> Result<(), String> {
        Ok(())
    }
}

impl Default for BranchesController {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Branch {
    pub name: String,
    pub full_ref_name: String,
    pub is_tracking_remote: bool,
    pub upstream_branch: Option<String>,
}

pub struct Binding {
    pub key: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct DisabledReason {
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub key: Option<char>,
    pub disabled_reason: Option<DisabledReason>,
}

pub struct RefreshOptions {
    pub mode: RefreshMode,
}

#[derive(Debug, Clone)]
pub enum RefreshMode {
    Sync,
    Async,
}

impl Default for RefreshOptions {
    fn default() -> Self {
        Self {
            mode: RefreshMode::Sync,
        }
    }
}

impl Default for DisabledReason {
    fn default() -> Self {
        Self {
            text: String::new(),
        }
    }
}

impl MenuItem {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            key: None,
            disabled_reason: None,
        }
    }
}

impl Branch {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            full_ref_name: name.to_string(),
            is_tracking_remote: false,
            upstream_branch: None,
        }
    }

    pub fn is_tracking_remote(&self) -> bool {
        self.is_tracking_remote
    }
}
