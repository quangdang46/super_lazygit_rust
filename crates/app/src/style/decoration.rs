// Ported from ./references/lazygit-master/pkg/gui/style/decoration.go

use ratatui::style::Modifier;

#[derive(Default, Clone, Copy)]
pub struct Decoration {
    bold: bool,
    underline: bool,
    reverse: bool,
    strikethrough: bool,
}

impl Decoration {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_bold(&mut self) {
        self.bold = true;
    }

    pub fn set_underline(&mut self) {
        self.underline = true;
    }

    pub fn set_reverse(&mut self) {
        self.reverse = true;
    }

    pub fn set_strikethrough(&mut self) {
        self.strikethrough = true;
    }

    pub fn to_modifiers(&self) -> Vec<Modifier> {
        let mut modifiers = Vec::with_capacity(4);
        if self.bold {
            modifiers.push(Modifier::BOLD);
        }
        if self.underline {
            modifiers.push(Modifier::UNDERLINED);
        }
        if self.reverse {
            modifiers.push(Modifier::REVERSED);
        }
        if self.strikethrough {
            modifiers.push(Modifier::CROSSED_OUT);
        }
        modifiers
    }

    pub fn merge(&self, other: &Decoration) -> Decoration {
        let mut result = *self;
        if other.bold {
            result.bold = true;
        }
        if other.underline {
            result.underline = true;
        }
        if other.reverse {
            result.reverse = true;
        }
        if other.strikethrough {
            result.strikethrough = true;
        }
        result
    }
}
