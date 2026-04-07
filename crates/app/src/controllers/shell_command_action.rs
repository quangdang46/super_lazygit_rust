// Ported from ./references/lazygit-master/pkg/gui/controllers/shell_command_action.go
use crate::controllers::ControllerCommon;

pub struct ShellCommandAction {
    common: ControllerCommon,
}

impl ShellCommandAction {
    pub fn new(common: ControllerCommon) -> Self {
        Self { common }
    }

    pub fn call(&self) -> Result<(), String> {
        Ok(())
    }

    pub fn get_shell_commands_history_suggestions_func(
        &self,
    ) -> Box<dyn Fn(String) -> Vec<Suggestion>> {
        Box::new(|_input| Vec::new())
    }

    fn should_save_command(&self, command: &str) -> bool {
        !command.starts_with(' ')
    }
}

pub struct Suggestion;
