// Ported from ./references/lazygit-master/pkg/gui/presentation/submodules.go

pub struct SubmoduleConfig {
    pub name: String,
    pub parent_module: Option<Box<SubmoduleConfig>>,
}

pub fn get_submodule_display_strings(s: &SubmoduleConfig) -> Vec<String> {
    let name = if let Some(ref _p) = s.parent_module {
        format!("  - {}", s.name)
    } else {
        s.name.clone()
    };
    vec![name]
}
