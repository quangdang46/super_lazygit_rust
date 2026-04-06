// Ported from ./references/lazygit-master/pkg/gui/controllers/tags_controller.go

pub struct TagsController {
    common: ControllerCommon,
    list_controller_trait: ListControllerTrait<Tag>,
}

pub struct Tag;

impl TagsController {
    pub fn new(common: ControllerCommon) -> Self {
        Self {
            common,
            list_controller_trait: ListControllerTrait::new(),
        }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn get_on_render_to_main(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }

    fn get_tag_info(&self, _tag: &Tag) -> String {
        String::new()
    }

    pub fn checkout(&self, _tag: &Tag) -> Result<(), String> {
        Ok(())
    }

    pub fn local_delete(&self, _tag: &Tag) -> Result<(), String> {
        Ok(())
    }

    pub fn remote_delete(&self, _tag: &Tag) -> Result<(), String> {
        Ok(())
    }

    pub fn local_and_remote_delete(&self, _tag: &Tag) -> Result<(), String> {
        Ok(())
    }

    pub fn delete(&self, _tag: &Tag) -> Result<(), String> {
        Ok(())
    }

    pub fn push(&self, _tag: &Tag) -> Result<(), String> {
        Ok(())
    }

    pub fn create_reset_menu(&self, _tag: &Tag) -> Result<(), String> {
        Ok(())
    }

    pub fn create(&self) -> Result<(), String> {
        Ok(())
    }

    fn context(&self) -> TagsContext {
        TagsContext
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

pub struct TagsContext;
pub struct ControllerCommon;
pub struct KeybindingsOpts;
pub struct Binding {
    pub key: char,
    pub description: String,
}
