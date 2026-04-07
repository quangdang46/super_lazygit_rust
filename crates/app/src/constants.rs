/// Documentation links for various features.
#[derive(Debug, Clone)]
pub struct Docs {
    pub custom_pagers: &'static str,
    pub custom_commands: &'static str,
    pub custom_keybindings: &'static str,
    pub keybindings: &'static str,
    pub undoing: &'static str,
    pub config: &'static str,
    pub tutorial: &'static str,
    pub custom_patch_demo: &'static str,
}

#[derive(Debug, Clone)]
pub struct Links {
    pub docs: Docs,
    pub issues: &'static str,
    pub donate: &'static str,
    pub discussions: &'static str,
    pub repo_url: &'static str,
    pub releases: &'static str,
}

pub const LINKS: Links = Links {
    issues: "https://github.com/jesseduffield/lazygit/issues",
    donate: "https://github.com/sponsors/jesseduffield",
    discussions: "https://github.com/jesseduffield/lazygit/discussions",
    repo_url: "https://github.com/jesseduffield/lazygit",
    releases: "https://github.com/jesseduffield/lazygit/releases",
    docs: Docs {
        custom_pagers: "https://github.com/jesseduffield/lazygit/blob/master/docs/Custom_Pagers.md",
        custom_keybindings:
            "https://github.com/jesseduffield/lazygit/blob/master/docs/keybindings/Custom_Keybindings.md",
        custom_commands: "https://github.com/jesseduffield/lazygit/wiki/Custom-Commands-Compendium",
        keybindings: "https://github.com/jesseduffield/lazygit/blob/%s/docs/keybindings",
        undoing: "https://github.com/jesseduffield/lazygit/blob/master/docs/Undoing.md",
        config: "https://github.com/jesseduffield/lazygit/blob/%s/docs/Config.md",
        tutorial: "https://youtu.be/VDXvbHZYeKY",
        custom_patch_demo:
            "https://github.com/jesseduffield/lazygit#rebase-magic-custom-patches",
    },
};
