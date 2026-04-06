// Ported from ./references/lazygit-master/pkg/gui/controllers/patch_building_controller.go

pub struct PatchBuildingController {
    common: ControllerCommon,
}

pub struct ControllerCommon;
pub struct ViewMouseBinding;
pub struct Binding;
pub struct KeybindingsOpts;
pub struct OnFocusOpts;
pub struct OnFocusLostOpts;
pub struct DisabledReason {
    pub text: String,
}

impl PatchBuildingController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn context(&self) -> PatchExplorerContext {
        PatchExplorerContext
    }

    pub fn get_mouse_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<ViewMouseBinding> {
        Vec::new()
    }

    pub fn get_on_focus(&self) -> Box<dyn Fn(OnFocusOpts)> {
        Box::new(|_| {})
    }

    pub fn get_on_focus_lost(&self) -> Box<dyn Fn(OnFocusLostOpts)> {
        Box::new(|_| {});
    }

    pub fn open_file(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn edit_file(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn toggle_selection_and_refresh(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn discard_selection(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn escape(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn escape_description(&self) -> String {
        String::new()
    }
}

pub struct PatchExplorerContext;
