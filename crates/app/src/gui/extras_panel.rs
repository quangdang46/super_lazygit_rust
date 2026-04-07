// Ported from ./references/lazygit-master/pkg/gui/extras_panel.go

pub struct Gui;

impl Gui {
    pub fn handle_create_extras_menu_panel(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn handle_focus_command_log(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn scroll_up_extra(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn scroll_down_extra(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn page_up_extras_panel(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn page_down_extras_panel(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn go_to_extras_panel_top(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn go_to_extras_panel_bottom(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn get_cmd_writer(&self) -> PrefixWriter {
        PrefixWriter::new()
    }
}

pub struct PrefixWriter {
    prefix: String,
    prefix_written: bool,
}

impl PrefixWriter {
    pub fn new() -> Self {
        Self {
            prefix: String::new(),
            prefix_written: false,
        }
    }

    pub fn write(&mut self, _data: &[u8]) -> (usize, Result<(), String>) {
        (0, Ok(()))
    }
}
