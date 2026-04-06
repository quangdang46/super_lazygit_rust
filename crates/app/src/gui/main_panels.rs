// Ported from ./references/lazygit-master/pkg/gui/main_panels.go

pub struct Gui;

impl Gui {
    pub fn run_task_for_view(&self, _view: &str, _task: &str) -> Result<(), String> {
        Ok(())
    }

    pub fn move_main_context_pair_to_top(&self, _pair: &str) {}

    pub fn move_main_context_to_top(&self, _context: &str) {}

    pub fn refresh_main_view(&self, _opts: &str, _context: &str) {}

    pub fn normal_main_context_pair(&self) -> String {
        String::new()
    }

    pub fn staging_main_context_pair(&self) -> String {
        String::new()
    }

    pub fn patch_building_main_context_pair(&self) -> String {
        String::new()
    }

    pub fn merging_main_context_pair(&self) -> String {
        String::new()
    }

    pub fn all_main_context_pairs(&self) -> Vec<String> {
        Vec::new()
    }

    pub fn refresh_main_views(&self, _opts: &str) {}

    pub fn split_main_panel(&self, _split_main_panel: bool) {}
}
