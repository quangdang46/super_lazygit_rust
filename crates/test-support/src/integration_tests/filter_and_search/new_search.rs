use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct NewSearchTest;

impl NewSearchTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Start a new search and verify the search begins from the current cursor position, not from the current search match".to_string(),
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

pub const NEW_SEARCH: NewSearchTest = NewSearchTest;
