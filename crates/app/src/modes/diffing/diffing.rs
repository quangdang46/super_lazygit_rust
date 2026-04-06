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

    pub fn get_from_and_reverse_args_for_diff(&self, from: &str) -> (&str, bool) {
        let reverse = false;
        let from_str;

        if self.active() {
            from_str = &self.ref_;
            (from_str, self.reverse)
        } else {
            from_str = from;
            (from_str, reverse)
        }
    }
}
