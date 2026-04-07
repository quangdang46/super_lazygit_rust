use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct NestedFilterTransientTest;

impl NestedFilterTransientTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Filter in a transient panel (sub-commits and diff-files) and ensure filter is cancelled when the panel is moved".to_string(),
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

pub const NESTED_FILTER_TRANSIENT: NestedFilterTransientTest = NestedFilterTransientTest;
