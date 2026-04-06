// Ported from ./references/lazygit-master/pkg/gui/types/modes.go

pub struct Modes {
    pub filtering: FilteringMode,
    pub cherry_picking: Option<CherryPickingMode>,
    pub diffing: DiffingMode,
    pub marked_base_commit: MarkedBaseCommitMode,
}

pub struct FilteringMode;
pub struct CherryPickingMode;
pub struct DiffingMode;
pub struct MarkedBaseCommitMode;
