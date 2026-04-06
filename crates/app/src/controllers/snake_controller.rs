// Ported from ./references/lazygit-master/pkg/gui/controllers/snake_controller.go

pub struct SnakeController {
    common: ControllerCommon,
}

pub struct ControllerCommon;

pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl SnakeController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn get_keybindings(&self, _opts: &KeybindingsOpts) -> Vec<Binding> {
        Vec::new()
    }

    pub fn context(&self) -> Option<Context> {
        None
    }

    pub fn get_on_focus(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }

    pub fn get_on_focus_lost(&self) -> Box<dyn Fn()> {
        Box::new(|| {})
    }

    pub fn set_direction(&self, _direction: Direction) -> Result<(), String> {
        Ok(())
    }

    pub fn escape(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct Context;
pub struct KeybindingsOpts;
pub struct Binding;
