// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/snake_helper.go

pub struct SnakeHelper {
    common: HelperCommon,
}

pub struct HelperCommon;

pub struct Game;

impl Game {
    pub fn new<F>(_width: i32, _height: i32, _render_callback: F, _log_action: fn(&str)) -> Self
    where
        F: Fn([[CellType; 0]; 0], bool) + Send + 'static,
    {
        Game
    }

    pub fn start(&mut self) {}
    pub fn exit(&mut self) {}
    pub fn set_direction(&mut self, _direction: Direction) {}
}

#[derive(Clone, Copy)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy)]
pub enum CellType {
    None,
    Snake,
    Food,
}

pub struct View;

impl View {
    pub fn inner_width(&self) -> i32 {
        0
    }
    pub fn inner_height(&self) -> i32 {
        0
    }
    pub fn clear(&self) {}
}

impl SnakeHelper {
    pub fn new(common: HelperCommon) -> Self {
        Self { common }
    }

    pub fn start_game(&mut self) {}

    pub fn exit_game(&mut self) {}

    pub fn set_direction(&mut self, direction: Direction) {}

    fn render_snake_game(&self, _cells: [[CellType; 0]; 0], _alive: bool) -> Result<(), String> {
        Ok(())
    }

    fn draw_snake_game(&self, _cells: [[CellType; 0]; 0]) -> String {
        String::new()
    }
}
