use std::io::Write;
use std::sync::RwLock;

use crate::style::text_style::TextStyle;

const MERGE_SYMBOL: char = '⏣';
const COMMIT_SYMBOL: char = '◯';

#[derive(Clone, Copy, PartialEq)]
enum CellType {
    Connection,
    Commit,
    Merge,
}

impl Default for CellType {
    fn default() -> Self {
        CellType::Connection
    }
}

struct Cell {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    cell_type: CellType,
    right_style: Option<TextStyle>,
    style: TextStyle,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            up: false,
            down: false,
            left: false,
            right: false,
            cell_type: CellType::Connection,
            right_style: None,
            style: TextStyle::new(),
        }
    }
}

impl Cell {
    fn render(&self, writer: &mut String) {
        let (first, second) = get_box_drawing_chars(self.up, self.down, self.left, self.right);

        let adjusted_first = match self.cell_type {
            CellType::Connection => first,
            CellType::Commit => COMMIT_SYMBOL.to_string(),
            CellType::Merge => MERGE_SYMBOL.to_string(),
        };

        let right_style = self.right_style.unwrap_or(self.style);

        let styled_second_char = if second == " " {
            " ".to_string()
        } else {
            cached_sprint(right_style, second)
        };

        writer.push_str(&cached_sprint(self.style, adjusted_first));
        writer.push_str(&styled_second_char);
    }

    fn reset(&mut self) {
        self.up = false;
        self.down = false;
        self.left = false;
        self.right = false;
    }

    fn set_up(&mut self, style: TextStyle) -> &mut Self {
        self.up = true;
        self.style = style;
        self
    }

    fn set_down(&mut self, style: TextStyle) -> &mut Self {
        self.down = true;
        self.style = style;
        self
    }

    fn set_left(&mut self, style: TextStyle) -> &mut Self {
        self.left = true;
        if !self.up && !self.down {
            self.style = style;
        }
        self
    }

    fn set_right(&mut self, style: TextStyle, override_style: bool) -> &mut Self {
        self.right = true;
        if self.right_style.is_none() || override_style {
            self.right_style = Some(style);
        }
        self
    }

    fn set_style(&mut self, style: TextStyle) -> &mut Self {
        self.style = style;
        self
    }

    fn set_type(&mut self, cell_type: CellType) -> &mut Self {
        self.cell_type = cell_type;
        self
    }
}

struct RgbCacheKey {
    r: u8,
    g: u8,
    b: u8,
    str: String,
}

impl PartialEq for RgbCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.r == other.r && self.g == other.g && self.b == other.b && self.str == other.str
    }
}

impl Eq for RgbCacheKey {}

impl std::hash::Hash for RgbCacheKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.r.hash(state);
        self.g.hash(state);
        self.b.hash(state);
        self.str.hash(state);
    }
}

static RGB_CACHE: RwLock<std::collections::HashMap<RgbCacheKey, String>> =
    RwLock::new(std::collections::HashMap::new());

fn cached_sprint(style: TextStyle, str: &str) -> String {
    let color = style.get_fg_color();
    if let Some((r, g, b)) = color {
        let key = RgbCacheKey {
            r,
            g,
            b,
            str: str.to_string(),
        };
        {
            let cache = RGB_CACHE.read().unwrap();
            if let Some(value) = cache.get(&key) {
                return value.clone();
            }
        }
        let value = style.sprint(str);
        {
            let mut cache = RGB_CACHE.write().unwrap();
            cache.insert(key, value.clone());
        }
        value
    } else {
        style.sprint(str)
    }
}

fn get_box_drawing_chars(up: bool, down: bool, left: bool, right: bool) -> (String, String) {
    match (up, down, left, right) {
        (true, true, true, true) => ("│".to_string(), "─".to_string()),
        (true, true, true, false) => ("│".to_string(), " ".to_string()),
        (true, true, false, true) => ("│".to_string(), "─".to_string()),
        (true, true, false, false) => ("│".to_string(), " ".to_string()),
        (true, false, true, true) => ("┴".to_string(), "─".to_string()),
        (true, false, true, false) => ("╯".to_string(), " ".to_string()),
        (true, false, false, true) => ("╰".to_string(), "─".to_string()),
        (true, false, false, false) => ("╵".to_string(), " ".to_string()),
        (false, true, true, true) => ("┬".to_string(), "─".to_string()),
        (false, true, true, false) => ("╮".to_string(), " ".to_string()),
        (false, true, false, true) => ("╭".to_string(), "─".to_string()),
        (false, true, false, false) => ("╷".to_string(), " ".to_string()),
        (false, false, true, true) => ("─".to_string(), "─".to_string()),
        (false, false, true, false) => ("─".to_string(), " ".to_string()),
        (false, false, false, true) => ("╶".to_string(), "─".to_string()),
        (false, false, false, false) => (" ".to_string(), " ".to_string()),
        _ => panic!("should not be possible"),
    }
}
