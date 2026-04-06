// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/tags_helper.go

pub struct TagsHelper {
    common: HelperCommon,
    commits_helper: CommitsHelper,
    gpg: GpgHelper,
}

pub struct HelperCommon;
pub struct CommitsHelper;
pub struct GpgHelper;

pub struct OpenCommitMessagePanelOpts {
    pub commit_index: i32,
    pub initial_message: String,
    pub summary_title: String,
    pub description_title: String,
    pub preserve_message: bool,
    pub on_confirm: fn(String, String) -> Result<(), String>,
}

impl TagsHelper {
    pub fn new(common: HelperCommon, commits_helper: CommitsHelper, gpg: GpgHelper) -> Self {
        Self {
            common,
            commits_helper,
            gpg,
        }
    }

    pub fn open_create_tag_prompt(&self, _ref: &str, _on_create: fn()) -> Result<(), String> {
        Ok(())
    }
}
