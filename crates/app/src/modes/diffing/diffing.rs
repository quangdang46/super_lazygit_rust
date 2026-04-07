// Ported from ./references/lazygit-master/pkg/gui/modes/diffing/diffing.go

#[derive(Clone, Default)]
pub struct Diffing {
    pub ref_: String,
    pub reverse: bool,
}

impl Diffing {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active(&self) -> bool {
        !self.ref_.is_empty()
    }

    pub fn get_from_and_reverse_args_for_diff(&self, from: &str) -> (String, bool) {
        let reverse = false;

        if self.active() {
            (self.ref_.clone(), self.reverse)
        } else {
            (from.to_string(), reverse)
        }
    }
}
