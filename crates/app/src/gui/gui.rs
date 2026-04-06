// Ported from ./references/lazygit-master/pkg/gui/gui.go

pub struct Gui {
    state: GuiRepoState,
}

pub struct GuiRepoState;

impl Gui {
    pub fn new() -> Self {
        Self {
            state: GuiRepoState,
        }
    }
}

pub struct StateAccessor;
pub struct PrevLayout;
pub struct Repo;
