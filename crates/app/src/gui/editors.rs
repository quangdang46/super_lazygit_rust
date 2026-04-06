// Ported from ./references/lazygit-master/pkg/gui/editors.go

pub struct Gui;

impl Gui {
    pub fn handle_editor_keypress(
        &self,
        _view: &str,
        _key: i32,
        _ch: char,
        _mod: i32,
        _allow_multiline: bool,
    ) -> bool {
        false
    }

    pub fn commit_message_editor(&self, _view: &str, _key: i32, _ch: char, _mod: i32) -> bool {
        false
    }

    pub fn commit_description_editor(&self, _view: &str, _key: i32, _ch: char, _mod: i32) -> bool {
        false
    }

    pub fn prompt_editor(&self, _view: &str, _key: i32, _ch: char, _mod: i32) -> bool {
        false
    }

    pub fn search_editor(&self, _view: &str, _key: i32, _ch: char, _mod: i32) -> bool {
        false
    }
}
