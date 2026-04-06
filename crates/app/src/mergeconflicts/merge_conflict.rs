// Ported from ./references/lazygit-master/pkg/gui/mergeconflicts/merge_conflict.go

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeConflict {
    pub start: i32,
    pub ancestor: i32,
    pub target: i32,
    pub end: i32,
}

impl MergeConflict {
    pub fn has_ancestor(&self) -> bool {
        self.ancestor >= 0
    }

    pub fn is_marker_line(&self, i: i32) -> bool {
        i == self.start || i == self.ancestor || i == self.target || i == self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Top,
    Middle,
    Bottom,
    All,
}

impl Selection {
    pub fn is_index_to_keep(&self, conflict: &MergeConflict, i: i32) -> bool {
        if i < conflict.start || conflict.end < i {
            return true;
        }

        if conflict.is_marker_line(i) {
            return false;
        }

        self.selected(conflict, i)
    }

    pub fn bounds(&self, c: &MergeConflict) -> (i32, i32) {
        match self {
            Selection::Top => {
                if c.has_ancestor() {
                    (c.start, c.ancestor)
                } else {
                    (c.start, c.target)
                }
            }
            Selection::Middle => (c.ancestor, c.target),
            Selection::Bottom => (c.target, c.end),
            Selection::All => (c.start, c.end),
        }
    }

    pub fn selected(&self, c: &MergeConflict, idx: i32) -> bool {
        let (start, end) = self.bounds(c);
        start < idx && idx < end
    }
}

pub fn available_selections(c: &MergeConflict) -> Vec<Selection> {
    if c.has_ancestor() {
        vec![Selection::Top, Selection::Middle, Selection::Bottom]
    } else {
        vec![Selection::Top, Selection::Bottom]
    }
}
