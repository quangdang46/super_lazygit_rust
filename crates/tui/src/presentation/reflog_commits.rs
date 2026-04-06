// Ported from ./references/lazygit-master/pkg/gui/presentation/reflog_commits.go

pub struct ReflogCommitDisplayAttributes {
    pub cherry_picked: bool,
    pub diffed: bool,
    pub parse_emoji: bool,
    pub time_format: String,
    pub short_time_format: String,
}

pub fn reflog_hash_color(cherry_picked: bool, diffed: bool) -> Style {
    if diffed {
        return Style::Cyan;
    }
    if cherry_picked {
        return Style::Magenta;
    }
    Style::Blue
}

pub fn get_full_description_display_strings_for_reflog_commit(
    _c: &Commit,
    attrs: &ReflogCommitDisplayAttributes,
) -> Vec<String> {
    let name = "reflog_entry".to_string();
    vec![
        format!("{:?}", reflog_hash_color(attrs.cherry_picked, attrs.diffed)),
        "time".to_string(),
        name,
    ]
}

pub fn get_display_strings_for_reflog_commit(
    _c: &Commit,
    attrs: &ReflogCommitDisplayAttributes,
) -> Vec<String> {
    let name = "reflog_entry".to_string();
    vec![
        format!("{:?}", reflog_hash_color(attrs.cherry_picked, attrs.diffed)),
        name,
    ]
}

pub struct Commit;

#[derive(Debug)]
pub enum Style {
    Cyan,
    Magenta,
    Blue,
}
