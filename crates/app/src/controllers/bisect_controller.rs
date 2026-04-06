use crate::controllers::ControllerCommon;

pub struct BisectController {
    common: ControllerCommon,
}

impl BisectController {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn get_keybindings(&self) -> Vec<Binding> {
        Vec::new()
    }

    pub fn open_menu(&self, _commit: &Commit) -> Result<(), String> {
        Ok(())
    }

    pub fn open_mid_bisect_menu(&self, _info: &BisectInfo, _commit: &Commit) -> Result<(), String> {
        Ok(())
    }

    pub fn open_start_bisect_menu(
        &self,
        _info: &BisectInfo,
        _commit: &Commit,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn show_bisect_complete_message(&self, _hashes: &[String]) -> Result<(), String> {
        Ok(())
    }

    pub fn after_mark(&self, _select_current: bool, _wait_to_reselect: bool) -> Result<(), String> {
        Ok(())
    }

    pub fn after_bisect_mark_refresh(
        &self,
        _select_current: bool,
        _wait_to_reselect: bool,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn select_current_bisect_commit(&self) {}
}

pub struct Commit {
    pub hash: String,
}

impl Commit {
    pub fn short_hash(&self) -> String {
        self.hash.chars().take(8).collect()
    }
}

pub struct BisectInfo {
    pub started: bool,
}

impl BisectInfo {
    pub fn new() -> Self {
        Self { started: false }
    }

    pub fn get_current_hash(&self) -> String {
        String::new()
    }

    pub fn new_term(&self) -> &str {
        "new"
    }

    pub fn old_term(&self) -> &str {
        "old"
    }
}

impl Default for BisectInfo {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Binding {
    pub key: String,
    pub description: String,
}

pub struct MenuItem {
    pub label: String,
    pub key: Option<char>,
}

pub struct CreateMenuOptions {
    pub title: String,
    pub items: Vec<MenuItem>,
}
