// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/sub_commits_helper.go

pub struct SubCommitsHelper {
    common: HelperCommon,
    refresh_helper: RefreshHelper,
}

pub struct HelperCommon;
pub struct RefreshHelper;

pub struct ViewSubCommitsOpts {
    pub r#ref: Ref,
    pub ref_to_show_divergence_from: String,
    pub title_ref: String,
    pub context: Context,
    pub show_branch_heads: bool,
}

pub struct Ref;
pub struct Commit;
pub struct Context;

impl Context {
    pub fn get_window_name(&self) -> String {
        String::new()
    }
}

impl SubCommitsHelper {
    pub fn new(common: HelperCommon, refresh_helper: RefreshHelper) -> Self {
        Self {
            common,
            refresh_helper,
        }
    }

    pub fn view_sub_commits(&self, _opts: ViewSubCommitsOpts) -> Result<(), String> {
        Ok(())
    }

    fn set_sub_commits(&self, _commits: Vec<Commit>) {}
}
