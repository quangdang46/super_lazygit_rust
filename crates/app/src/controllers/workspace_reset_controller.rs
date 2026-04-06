// Ported from ./references/lazygit-master/pkg/gui/controllers/workspace_reset_controller.go

pub struct FilesController {
    common: ControllerCommon,
}

impl FilesController {
    pub fn create_reset_menu(&self) -> Result<(), String> {
        Ok(())
    }

    fn animate_explosion(&self) {}

    fn explode(&self, _view: &str, _on_done: fn()) {}
}

fn get_explode_image(_width: i32, _height: i32, _frame: i32, _max: i32) -> String {
    String::new()
}

pub struct ControllerCommon;
