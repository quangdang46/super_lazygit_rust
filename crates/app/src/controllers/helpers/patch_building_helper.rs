// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/patch_building_helper.go

pub struct PatchBuildingHelper {
    context: HelperCommon,
}

pub struct HelperCommon;

impl PatchBuildingHelper {
    pub fn new(context: HelperCommon) -> Self {
        Self { context }
    }

    pub fn show_hunk_staging_hint(&self) {}

    pub fn escape(&self) {}

    pub fn reset(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn refresh_patch_building_panel(&self, _opts: &OnFocusOpts) {}
}

pub struct OnFocusOpts;

impl PatchBuildingHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
        }
    }
}

impl Default for PatchBuildingHelper {
    fn default() -> Self {
        Self::new()
    }
}
