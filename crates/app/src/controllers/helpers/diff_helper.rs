// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/diff_helper.go

pub struct Commit {
    pub hash: String,
    pub filter_paths: Vec<String>,
}

impl Commit {
    pub fn hash(&self) -> &str {
        &self.hash
    }
}

pub struct RefRange {
    pub from: Box<dyn Ref>,
    pub to: Box<dyn Ref>,
}

pub trait Ref {
    fn ref_name(&self) -> String;
    fn short_ref_name(&self) -> String;
    fn parent_ref_name(&self) -> String;
}

pub struct RefRangeRef;

impl Ref for RefRangeRef {
    fn ref_name(&self) -> String {
        String::new()
    }
    fn short_ref_name(&self) -> String {
        String::new()
    }
    fn parent_ref_name(&self) -> String {
        String::new()
    }
}

pub struct UpdateTask;

pub struct RefreshMainOpts {
    pub pair: MainViewPairs,
    pub main: ViewUpdateOpts,
}

pub struct MainViewPairs;

pub struct ViewUpdateOpts {
    pub title: String,
    pub sub_title: String,
    pub task: UpdateTask,
}

pub struct DiffToolCmdOptions {
    pub filepath: String,
    pub from_commit: String,
    pub to_commit: String,
    pub reverse: bool,
    pub is_directory: bool,
    pub staged: bool,
}

pub struct HelperCommon;

pub struct DiffHelper {
    context: HelperCommon,
}

impl DiffHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
        }
    }

    pub fn diff_args(&self) -> Vec<String> {
        vec!["--stat".to_string(), "-p".to_string()]
    }

    pub fn get_update_task_for_rendering_commits_diff(
        &self,
        _commit: &Commit,
        _ref_range: Option<&RefRange>,
    ) -> UpdateTask {
        UpdateTask
    }

    pub fn filter_paths_for_commit(&self, _commit: &Commit) -> Vec<String> {
        Vec::new()
    }

    pub fn exit_diff_mode(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn render_diff(&self) {}

    pub fn current_diff_terminals(&self) -> Vec<String> {
        Vec::new()
    }

    pub fn current_diff_terminal(&self) -> String {
        String::new()
    }

    pub fn currently_selected_filename(&self) -> String {
        String::new()
    }

    pub fn with_diff_mode_check<F>(&self, _f: F)
    where
        F: Fn(),
    {
    }

    pub fn ignoring_whitespace_sub_title(&self) -> String {
        String::new()
    }

    pub fn open_diff_tool_for_ref(&self, _selected_ref: &dyn Ref) -> Result<(), String> {
        Ok(())
    }

    pub fn adjust_line_number(&self, _path: &str, linenumber: i64, _viewname: &str) -> i64 {
        linenumber
    }

    fn adjust_line_number_internal(&self, linenumber: i64, _diff_args: &[String]) -> i64 {
        linenumber
    }
}

impl Default for DiffHelper {
    fn default() -> Self {
        Self::new()
    }
}
