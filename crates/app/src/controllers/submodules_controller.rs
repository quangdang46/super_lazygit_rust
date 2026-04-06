// Ported from ./references/lazygit-master/pkg/gui/controllers/submodules_controller.go

pub struct SubmodulesController {
    common: ControllerCommon,
    list_controller_trait: ListControllerTrait<SubmoduleConfig>,
}

pub struct SubmoduleConfig;

impl SubmodulesController {
    pub fn new(common: ControllerCommon) -> Self {
        Self {
            common,
            list_controller_trait: ListControllerTrait::new(),
        }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_on_double_click(&self) -> Box<dyn Fn() -> Result<(), String>> {
        Box::new(|| Ok(()))
    }

    pub fn get_on_render_to_main(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }

    pub fn enter(&self, _submodule: &SubmoduleConfig) -> Result<(), String> {
        Ok(())
    }

    pub fn add(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn edit_url(&self, _submodule: &SubmoduleConfig) -> Result<(), String> {
        Ok(())
    }

    pub fn init(&self, _submodule: &SubmoduleConfig) -> Result<(), String> {
        Ok(())
    }

    pub fn open_bulk_actions_menu(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn update(&self, _submodule: &SubmoduleConfig) -> Result<(), String> {
        Ok(())
    }

    pub fn remove(&self, _submodule: &SubmoduleConfig) -> Result<(), String> {
        Ok(())
    }

    pub fn easter_egg(&self) -> Result<(), String> {
        Ok(())
    }

    fn context(&self) -> SubmodulesContext {
        SubmodulesContext
    }
}

pub struct ListControllerTrait<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T> ListControllerTrait<T> {
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

pub struct SubmodulesContext;
pub struct ControllerCommon;
pub struct KeybindingsOpts;
pub struct Binding {
    pub key: char,
    pub description: String,
}
