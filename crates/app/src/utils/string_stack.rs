// Ported from ./references/lazygit-master/pkg/utils/string_stack.go

pub struct StringStack {
    stack: Vec<String>,
}

impl StringStack {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn push(&mut self, s: String) {
        self.stack.push(s);
    }

    pub fn pop(&mut self) -> String {
        if self.stack.is_empty() {
            return String::new();
        }
        self.stack.pop().unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }
}

impl Default for StringStack {
    fn default() -> Self {
        Self::new()
    }
}
