// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/commits_helper.go

pub struct Suggestion;

pub struct OpenCommitMessagePanelOpts {
    pub commit_index: i64,
    pub summary_title: String,
    pub description_title: String,
    pub preserve_message: bool,
    pub initial_message: String,
    pub force_skip_hooks: bool,
    pub skip_hooks_prefix: String,
}

pub struct HelperCommon;

pub struct CommitsHelper {
    context: HelperCommon,
    get_commit_summary: fn() -> String,
    set_commit_summary: fn(String),
    get_commit_description: fn() -> String,
    get_unwrapped_commit_description: fn() -> String,
    set_commit_description: fn(String),
}

impl CommitsHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
            get_commit_summary: || String::new(),
            set_commit_summary: |_| {},
            get_commit_description: || String::new(),
            get_unwrapped_commit_description: || String::new(),
            set_commit_description: |_| {},
        }
    }

    pub fn split_commit_message_and_description(&self, message: &str) -> (String, String) {
        let parts: Vec<&str> = message.splitn(2, '\n').collect();
        let msg = parts.first().unwrap_or(&"").to_string();
        let description = parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();
        (msg, description)
    }

    pub fn set_message_and_description_in_view(&self, message: &str) {}

    pub fn join_commit_message_and_unwrapped_description(&self) -> String {
        String::new()
    }

    pub fn try_remove_hard_line_breaks(message: &str, _auto_wrap_width: i64) -> String {
        message.to_string()
    }

    pub fn switch_to_editor(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn update_commit_panel_view(&self, _message: &str) {}

    pub fn open_commit_message_panel(&self, _opts: &OpenCommitMessagePanelOpts) {}

    pub fn clear_preserved_commit_message(&self) {}

    pub fn handle_commit_confirm(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn close_commit_message_panel(&self) {}

    pub fn open_commit_menu(
        &self,
        _suggestion_func: fn(&str) -> Vec<Suggestion>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn add_co_author(&self, _suggestion_func: fn(&str) -> Vec<Suggestion>) -> Result<(), String> {
        Ok(())
    }

    fn paste_commit_message_from_clipboard(&self) -> Result<(), String> {
        Ok(())
    }
}

impl Default for CommitsHelper {
    fn default() -> Self {
        Self::new()
    }
}
