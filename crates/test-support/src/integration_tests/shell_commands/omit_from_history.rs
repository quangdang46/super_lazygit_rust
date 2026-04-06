use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct OmitFromHistoryTest;

impl OmitFromHistoryTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Omitting a runtime custom command from history if it begins with space"
                .to_string(),
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
                    .r#type("echo aubergine")
                    .confirm();

                t.global_press(&keys.universal().execute_shell_command);

                t.expect_popup().prompt().title("Shell command:").confirm();

                t.global_press(&keys.universal().execute_shell_command);

                t.expect_popup().prompt().title("Shell command:").cancel();
            })),
            extra_env_vars: std::collections::HashMap::new(),
            git_version: Default::default(),
            width: 0,
            height: 0,
            is_demo: false,
        })
    }
}

pub const OMIT_FROM_HISTORY: OmitFromHistoryTest = OmitFromHistoryTest;
