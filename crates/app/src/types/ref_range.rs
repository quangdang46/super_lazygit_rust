// Ported from ./references/lazygit-master/pkg/gui/types/ref_range.go

pub struct RefRange {
    pub from: Ref,
    pub to: Ref,
}

pub struct Ref {
    pub name: String,
    pub hash: String,
}
