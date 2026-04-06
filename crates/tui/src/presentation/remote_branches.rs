// Ported from ./references/lazygit-master/pkg/gui/presentation/remote_branches.go

#[derive(Debug)]
pub enum Style {
    Cyan,
    Default,
}

pub struct RemoteBranch {
    pub name: String,
}

impl RemoteBranch {
    pub fn full_name(&self) -> &str {
        &self.name
    }
}

pub fn get_remote_branch_display_strings(b: &RemoteBranch, diffed: bool) -> Vec<String> {
    let name_style = if diffed { Style::Cyan } else { Style::Default };

    let mut result = Vec::with_capacity(2);
    result.push(format!("{:?}", name_style));
    result.push(b.name.clone());
    result
}
