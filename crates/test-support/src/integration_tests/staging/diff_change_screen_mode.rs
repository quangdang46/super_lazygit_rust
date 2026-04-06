use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct DiffChangeScreenModeTest;

impl DiffChangeScreenModeTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Change the staged changes screen mode".to_string(),
            extra_cmd_args: vec![],
            skip: false,
            setup_repo: Some(Box::new(|shell| {
                shell.create_file("file", "first line\nsecond line");
            })),
            setup_config: Some(Box::new(|_cfg| {})),
            run: Some(Box::new(|_t, _keys| {})),
            extra_env_vars: std::collections::HashMap::new(),
            git_version: Default::default(),
            width: 0,
            height: 0,
            is_demo: false,
        })
    }
}

pub const DIFF_CHANGE_SCREEN_MODE: DiffChangeScreenModeTest = DiffChangeScreenModeTest;
