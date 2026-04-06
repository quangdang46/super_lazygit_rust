// Ported from ./references/lazygit-master/pkg/gui/types/suggestion.go

pub struct Suggestion {
    pub value: String,
    pub label: String,
}

impl Suggestion {
    pub fn id(&self) -> String {
        self.value.clone()
    }
}
