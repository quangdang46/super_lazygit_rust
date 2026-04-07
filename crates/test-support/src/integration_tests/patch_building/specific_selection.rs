use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct SpecificSelectionTest;

impl SpecificSelectionTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Build a custom patch with a specific selection of lines, adding individual lines, as well as a range and hunk, and adding a file directly".to_string(),
            extra_cmd_args: vec![],
            skip: false,
            setup_repo: Some(Box::new(|_shell| {})),
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

pub const SPECIFIC_SELECTION: SpecificSelectionTest = SpecificSelectionTest;
