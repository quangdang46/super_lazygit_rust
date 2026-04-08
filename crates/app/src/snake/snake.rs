// Ported from ./references/lazygit-master/pkg/snake/snake.go

use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Duration;

/// Direction of the snake
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Cell type on the board
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CellType {
    None,
    Snake,
    Food,
}

/// Position on the board
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

/// State of the game
#[derive(Clone, Debug)]
pub struct State {
    /// First element is the head, final element is the tail
    pub snake_positions: Vec<Position>,
    pub food_position: Position,
    /// Direction of the snake
    pub direction: Direction,
    /// Direction as of the end of the last tick
    pub last_tick_direction: Direction,
}

/// Render callback type
pub type RenderCallback = Box<dyn Fn(&[Vec<CellType>], bool) + Send + 'static>;

/// Logger callback type
pub type LoggerCallback = Box<dyn Fn(&str) + Send + 'static>;

/// Game struct
pub struct Game {
    /// Width/height of the board
    width: i32,
    height: i32,
    /// Function for rendering the game. If alive is false, the cells are expected to be ignored.
    render: Option<RenderCallback>,
    /// Closed when the game is exited
    exit_tx: Option<Sender<()>>,
    /// Channel for specifying the direction the player wants the snake to go in
    new_dir_tx: Option<Sender<Direction>>,
    /// Allows logging for debugging
    logger: Option<LoggerCallback>,
}

impl Game {
    /// Create a new game
    pub fn new<F1, F2>(width: i32, height: i32, render: F1, logger: F2) -> Self
    where
        F1: Fn(&[Vec<CellType>], bool) + Send + 'static,
        F2: Fn(&str) + Send + 'static,
    {
        Self {
            width,
            height,
            render: Some(Box::new(render)),
            exit_tx: None,
            new_dir_tx: None,
            logger: Some(Box::new(logger)),
        }
    }

    /// Start the game
    pub fn start(&mut self) {
        let (exit_tx, _exit_rx) = channel();
        let (new_dir_tx, new_dir_rx) = channel();

        self.exit_tx = Some(exit_tx);
        self.new_dir_tx = Some(new_dir_tx);

        let width = self.width;
        let height = self.height;
        let render = self.render.take();
        let logger = self.logger.take();

        thread::spawn(move || {
            let mut state = Self::initialize_state(width, height);
            let alive = true;
            let cells = Self::get_cells_static(&state, width, height);

            if let Some(render_fn) = &render {
                render_fn(&cells, alive);
            }

            if let Some(logger_fn) = &logger {
                logger_fn("Snake game started");
            }

            // Using a simple timer instead of tokio
            let tick_interval = Duration::from_millis(75);
            let mut last_tick = std::time::Instant::now();

            loop {
                // Check for new direction
                if let Ok(dir) = new_dir_rx.try_recv() {
                    state.direction = Self::new_direction(&state, dir);
                }

                // Check if tick interval has passed
                if last_tick.elapsed() >= tick_interval {
                    let (new_state, alive) = Self::tick_static(state, width, height);
                    state = new_state;
                    last_tick = std::time::Instant::now();

                    let cells = Self::get_cells_static(&state, width, height);
                    if let Some(render_fn) = &render {
                        render_fn(&cells, alive);
                    }

                    if !alive {
                        if let Some(logger_fn) = &logger {
                            logger_fn("Snake game over");
                        }
                        return;
                    }
                }

                // Small sleep to prevent busy waiting
                thread::sleep(Duration::from_millis(10));
            }
        });
    }

    /// Exit the game
    pub fn exit(&self) {
        if let Some(tx) = &self.exit_tx {
            let _ = tx.send(());
        }
    }

    /// Set the direction
    pub fn set_direction(&self, direction: Direction) {
        if let Some(tx) = &self.new_dir_tx {
            let _ = tx.send(direction);
        }
    }

    /// Initialize the game state
    fn initialize_state(width: i32, height: i32) -> State {
        let center_of_screen = Position {
            x: width / 2,
            y: height / 2,
        };
        let snake_positions = vec![center_of_screen];

        let mut state = State {
            snake_positions,
            direction: Direction::Right,
            food_position: Position { x: 0, y: 0 },
            last_tick_direction: Direction::Right,
        };

        state.food_position = Self::new_food_pos_static(&state.snake_positions, width, height);

        state
    }

    /// Generate a new food position
    fn new_food_pos_static(snake_positions: &[Position], width: i32, height: i32) -> Position {
        let attempt_limit = 1000;

        // Simple random-like position using system time
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i32;

        for i in 0..attempt_limit {
            let x = ((seed + i) % width).abs();
            let y = ((seed + i + 100) % height).abs();
            let new_food_pos = Position { x, y };

            if !snake_positions.contains(&new_food_pos) {
                return new_food_pos;
            }
        }

        panic!("SORRY, BUT I WAS TOO LAZY TO MAKE THE SNAKE GAME SMART ENOUGH TO PUT THE FOOD SOMEWHERE SENSIBLE NO MATTER WHAT, AND I ALSO WAS TOO LAZY TO ADD A WIN CONDITION");
    }

    /// Tick - returns whether the snake is alive
    fn tick_static(state: State, width: i32, height: i32) -> (State, bool) {
        let mut next_state = state.clone();
        let mut new_head_pos = next_state.snake_positions[0];

        next_state.last_tick_direction = next_state.direction;

        match next_state.direction {
            Direction::Up => new_head_pos.y -= 1,
            Direction::Down => new_head_pos.y += 1,
            Direction::Left => new_head_pos.x -= 1,
            Direction::Right => new_head_pos.x += 1,
        }

        let out_of_bounds = new_head_pos.x < 0
            || new_head_pos.x >= width
            || new_head_pos.y < 0
            || new_head_pos.y >= height;

        let eating_own_tail = next_state.snake_positions.contains(&new_head_pos);

        if out_of_bounds || eating_own_tail {
            return (State::default(), false);
        }

        next_state.snake_positions.insert(0, new_head_pos);

        if new_head_pos == next_state.food_position {
            next_state.food_position =
                Self::new_food_pos_static(&next_state.snake_positions, width, height);
        } else {
            next_state.snake_positions.pop();
        }

        (next_state, true)
    }

    /// Get cells for rendering
    fn get_cells_static(state: &State, width: i32, height: i32) -> Vec<Vec<CellType>> {
        let mut cells: Vec<Vec<CellType>> = vec![vec![CellType::None; width as usize]; height as usize];

        for pos in &state.snake_positions {
            if pos.y >= 0 && pos.y < height && pos.x >= 0 && pos.x < width {
                cells[pos.y as usize][pos.x as usize] = CellType::Snake;
            }
        }

        if state.food_position.y >= 0 && state.food_position.y < height
            && state.food_position.x >= 0 && state.food_position.x < width {
            cells[state.food_position.y as usize][state.food_position.x as usize] = CellType::Food;
        }

        cells
    }

    /// Calculate new direction (prevent 180 degree turns)
    fn new_direction(state: &State, direction: Direction) -> Direction {
        let forbidden = match state.last_tick_direction {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        };

        if direction == forbidden {
            state.direction
        } else {
            direction
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            snake_positions: Vec::new(),
            food_position: Position { x: 0, y: 0 },
            direction: Direction::Right,
            last_tick_direction: Direction::Right,
        }
    }
}
