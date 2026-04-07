// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/mode_helper.go

pub struct ModeHelper {
    context: HelperCommon,
    diff_helper: DiffHelper,
    patch_building_helper: PatchBuildingHelper,
    cherry_pick_helper: CherryPickHelper,
    merge_and_rebase_helper: MergeAndRebaseHelper,
    bisect_helper: BisectHelper,
    suppress_rebasing_mode: bool,
}

pub struct HelperCommon;
pub struct DiffHelper;
pub struct PatchBuildingHelper;
pub struct CherryPickHelper;
pub struct MergeAndRebaseHelper;
pub struct BisectHelper;

pub struct ModeStatus {
    pub is_active: fn() -> bool,
    pub info_label: fn() -> String,
    pub cancel_label: fn() -> String,
    pub reset: fn() -> Result<(), String>,
}

impl ModeHelper {
    pub fn new(
        context: HelperCommon,
        diff_helper: DiffHelper,
        patch_building_helper: PatchBuildingHelper,
        cherry_pick_helper: CherryPickHelper,
        merge_and_rebase_helper: MergeAndRebaseHelper,
        bisect_helper: BisectHelper,
    ) -> Self {
        Self {
            context,
            diff_helper,
            patch_building_helper,
            cherry_pick_helper,
            merge_and_rebase_helper,
            bisect_helper,
            suppress_rebasing_mode: false,
        }
    }

    pub fn statuses(&self) -> Vec<ModeStatus> {
        Vec::new()
    }

    fn with_reset_button(&self, content: &str) -> String {
        content.to_string()
    }

    pub fn get_active_mode(&self) -> Option<ModeStatus> {
        None
    }

    pub fn is_any_mode_active(&self) -> bool {
        false
    }

    pub fn exit_filter_mode(&self) -> Result<(), String> {
        self.clear_filtering()
    }

    pub fn clear_filtering(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn set_suppress_rebasing_mode(&self, _value: bool) {}
}
