// Ported from ./references/lazygit-master/pkg/gui/presentation/tags.go

use super::item_operations::ItemOperation;

pub struct Tag {
    pub name: String,
}

impl Tag {
    pub fn description(&self) -> String {
        "tag_description".to_string()
    }
}

pub struct TagDisplayOptions<'a> {
    pub tag: &'a Tag,
    pub item_operation: ItemOperation,
    pub diffed: bool,
}

pub fn get_tag_display_strings(opts: TagDisplayOptions) -> Vec<String> {
    let text_style = if opts.diffed { "Cyan" } else { "Default" };
    let description_style = "Yellow";

    let description = opts.tag.description();
    vec![
        format!("{:?}", text_style),
        format!("{:?} {}", description_style, description),
    ]
}
