use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct ComplexShellCommandTest;

impl ComplexShellCommandTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Using a custom command provided at runtime to create a new file, via a shell command".to_string(),
            extra_cmd_args: vec![],
            skip: false,
            setup_repo: Some(Box::new(|shell| {
                shell.empty_commit("blah");
            })),
            setup_config: Some(Box::new(|_cfg| {})),
            run: Some(Box::new(|t, keys| {
                t.global_press(&keys.universal().execute_shell_command);

                t.expect_popup()
                    .prompt()
                    .title("Shell command:")
                    .r#type("sh -c \"touch file.txt\"")
                    .confirm();

                t.global_press(&keys.universal().execute_shell_command);

                t.expect_popup()
                    .prompt()
                    .title("Shell command:")
                    .confirm();
            })),
            extra_env_vars: std::collections::HashMap::new(),
            git_version: Default::default(),
            width: 0,
            height: 0,
            is_demo: false,
        })
    }
}

pub const COMPLEX_SHELL_COMMAND: ComplexShellCommandTest = ComplexShellCommandTest;
