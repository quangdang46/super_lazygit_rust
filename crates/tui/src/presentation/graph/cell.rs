// Ported from ./references/lazygit-master/pkg/gui/presentation/graph/cell.go

pub const MERGE_SYMBOL: char = '⏣';
pub const COMMIT_SYMBOL: char = '◯';

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CellType {
    Connection,
    Commit,
    Merge,
}

impl Default for CellType {
    fn default() -> Self {
        CellType::Connection
    }
}

#[derive(Clone, Default)]
pub struct Cell {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub cell_type: CellType,
    pub right_style: Option<Style>,
    pub style: Option<Style>,
}

#[derive(Clone)]
pub struct Style;

impl Style {
    pub fn new() -> Self {
        Style
    }
}

impl Cell {
    pub fn render(&self) -> String {
        let (first, second) = get_box_drawing_chars(self.up, self.down, self.left, self.right);

        let adjusted_first = match self.cell_type {
            CellType::Connection => first.to_string(),
            CellType::Commit => COMMIT_SYMBOL.to_string(),
            CellType::Merge => MERGE_SYMBOL.to_string(),
        };

        let right_style = self
            .right_style
            .clone()
            .unwrap_or_else(|| self.style.clone().unwrap_or_else(Style::new));

        let styled_second_char = if second == " " {
            " ".to_string()
        } else {
            cached_sprint(&right_style, second)
        };

        let styled_first = cached_sprint(
            &self.style.clone().unwrap_or_else(Style::new),
            &adjusted_first,
        );
        format!("{}{}", styled_first, styled_second_char)
    }

    pub fn reset(&mut self) {
        self.up = false;
        self.down = false;
        self.left = false;
        self.right = false;
    }

    pub fn set_up(&mut self, style: Option<Style>) -> &mut Self {
        self.up = true;
        self.style = style;
        self
    }

    pub fn set_down(&mut self, style: Option<Style>) -> &mut Self {
        self.down = true;
        self.style = style;
        self
    }

    pub fn set_left(&mut self, style: Option<Style>) -> &mut Self {
        self.left = true;
        if !self.up && !self.down {
            self.style = style.clone();
        }
        self
    }

    pub fn set_right(&mut self, style: Option<Style>, override_style: bool) -> &mut Self {
        self.right = true;
        if self.right_style.is_none() || override_style {
            self.right_style = style;
        }
        self
    }

    pub fn set_style(&mut self, style: Option<Style>) -> &mut Self {
        self.style = style;
        self
    }

    pub fn set_type(&mut self, cell_type: CellType) -> &mut Self {
        self.cell_type = cell_type;
        self
    }
}

pub fn get_box_drawing_chars(
    up: bool,
    down: bool,
    left: bool,
    right: bool,
) -> (&'static str, &'static str) {
    if up && down && left && right {
        ("│", "─")
    } else if up && down && left && !right {
        ("│", " ")
    } else if up && down && !left && right {
        ("│", "─")
    } else if up && down && !left && !right {
        ("│", " ")
    } else if up && !down && left && right {
        ("┴", "─")
    } else if up && !down && left && !right {
        ("╯", " ")
    } else if up && !down && !left && right {
        ("╰", "─")
    } else if up && !down && !left && !right {
        ("╵", " ")
    } else if !up && down && left && right {
        ("┬", "─")
    } else if !up && down && left && !right {
        ("╮", " ")
    } else if !up && down && !left && right {
        ("╭", "─")
    } else if !up && down && !left && !right {
        ("╷", " ")
    } else if !up && !down && left && right {
        ("─", "─")
    } else if !up && !down && left && !right {
        ("─", " ")
    } else if !up && !down && !left && right {
        ("╶", "─")
    } else if !up && !down && !left && !right {
        (" ", " ")
    } else {
        panic!("should not be possible")
    }
}

pub fn cached_sprint(_style: &Style, s: &str) -> String {
    s.to_string()
}
