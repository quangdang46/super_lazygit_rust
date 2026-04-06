// Ported from ./references/lazygit-master/pkg/gui/presentation/remotes.go

use super::item_operations::ItemOperation;

pub struct Remote {
    pub name: String,
    pub branches: Vec<String>,
}

pub struct RemoteDisplayOptions<'a> {
    pub remote: &'a Remote,
    pub diffed: bool,
    pub item_operation: ItemOperation,
    pub branch_count: usize,
}

pub fn get_remote_display_strings(opts: RemoteDisplayOptions) -> Vec<String> {
    let branch_count = opts.remote.branches.len();

    let text_style = if opts.diffed { "Cyan" } else { "Default" };

    let mut result = Vec::with_capacity(3);
    result.push(format!("{:?}", text_style));

    let description = format!("{} branches", branch_count);
    result.push(description);
    result
}
