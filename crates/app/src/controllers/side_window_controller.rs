// Ported from ./references/lazygit-master/pkg/gui/controllers/side_window_controller.go

pub struct SideWindowControllerFactory {
    common: ControllerCommon,
}

pub struct SideWindowController {
    common: ControllerCommon,
    context: Context,
}

pub struct ControllerCommon;
pub struct Context;

impl SideWindowControllerFactory {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn create(&self, context: Context) -> SideWindowController {
        SideWindowController {
            common: self.common,
            context,
        }
    }
}

impl SideWindowController {
    pub fn new(common: ControllerCommon, context: Context) -> Self {
        Self { common, context }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn context(&self) -> Option<Context> {
        None
    }

    pub fn previous_side_window(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn next_side_window(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct KeybindingsOpts;
pub struct Binding;
