// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/repos_helper.go

use std::fs;
use std::path::Path;
use std::sync::Arc;

pub struct ReposHelper {
    common: HelperCommon,
    record_directory_helper: Arc<RecordDirectoryHelper>,
    on_new_repo: Box<dyn Fn(StartArgs, String) -> Result<(), String> + Send + Sync>,
}

pub struct HelperCommon;
pub struct RecordDirectoryHelper;

impl RecordDirectoryHelper {
    pub fn record_current_directory(&self) {}
}

pub struct StartArgs;
pub struct SubmoduleConfig;

impl SubmoduleConfig {
    pub fn full_path(&self) -> String {
        String::new()
    }
}

pub struct RepoPathStack;

impl RepoPathStack {
    pub fn push(&self, _path: &str) {}
    pub fn clear(&self) {}
}

impl ReposHelper {
    pub fn new(
        common: HelperCommon,
        record_directory_helper: Arc<RecordDirectoryHelper>,
        on_new_repo: Box<dyn Fn(StartArgs, String) -> Result<(), String> + Send + Sync>,
    ) -> Self {
        Self {
            common,
            record_directory_helper,
            on_new_repo,
        }
    }

    pub fn enter_submodule(&self, _submodule: &SubmoduleConfig) -> Result<(), String> {
        Ok(())
    }

    pub fn get_current_branch(&self, path: &str) -> String {
        let head_file_path = Path::new(path).join(".git").join("HEAD");

        if let Ok(content) = fs::read_to_string(&head_file_path) {
            let content = content.trim();
            if let Some(branch) = content.strip_prefix("ref: refs/heads/") {
                return branch.to_string();
            }
        }

        "Unknown".to_string()
    }

    pub fn create_recent_repos_menu(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn dispatch_switch_to_repo(&self, path: &str, context_key: String) -> Result<(), String> {
        self.dispatch_switch_to(path, "Repository not found", context_key)
    }

    pub fn dispatch_switch_to(
        &self,
        _path: &str,
        _err_msg: &str,
        _context_key: String,
    ) -> Result<(), String> {
        self.record_directory_helper.record_current_directory();
        Ok(())
    }
}

pub struct MenuItem {
    pub label: String,
}

pub struct CreateMenuOptions {
    pub title: String,
    pub items: Vec<MenuItem>,
}

pub struct Branch;

pub struct RecentReposHelper;

impl RecentReposHelper {
    pub fn new(
        common: HelperCommon,
        record_directory_helper: Arc<RecordDirectoryHelper>,
        on_new_repo: Box<dyn Fn(StartArgs, String) -> Result<(), String> + Send + Sync>,
    ) -> ReposHelper {
        ReposHelper::new(common, record_directory_helper, on_new_repo)
    }
}
