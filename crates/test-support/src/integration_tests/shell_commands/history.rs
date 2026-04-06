use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct HistoryTest;

impl HistoryTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Test that the custom commands history is saved correctly".to_string(),
            extra_cmd_args: vec![],
            skip: false,
            setup_repo: Some(Box::new(|_shell| {})),
            setup_config: Some(Box::new(|_cfg| {})),
            run: Some(Box::new(|t, keys| {
                t.global_press(&keys.universal().execute_shell_command);

                t.expect_popup()
                    .prompt()
                    .title("Shell command:")
                    .r#type("echo 1")
                    .confirm();

                t.global_press(&keys.universal().execute_shell_command);

                t.expect_popup().prompt().title("Shell command:").confirm();

                t.global_press(&keys.universal().execute_shell_command);

                t.expect_popup().prompt().title("Shell command:").confirm();

                t.global_press(&keys.universal().execute_shell_command);

                t.expect_popup().prompt().title("Shell command:").confirm();

                t.global_press(&keys.universal().execute_shell_command);

                t.expect_popup().prompt().title("Shell command:").confirm();
            })),
            extra_env_vars: std::collections::HashMap::new(),
            git_version: Default::default(),
            width: 0,
            height: 0,
            is_demo: false,
        })
    }
}

pub const HISTORY: HistoryTest = HistoryTest;
