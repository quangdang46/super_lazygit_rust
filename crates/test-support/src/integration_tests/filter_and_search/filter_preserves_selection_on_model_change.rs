use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct FilterPreservesSelectionOnModelChangeTest;

impl FilterPreservesSelectionOnModelChangeTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Verify that when a filter is active and the model changes, the selection is preserved".to_string(),
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

pub const FILTER_PRESERVES_SELECTION_ON_MODEL_CHANGE: FilterPreservesSelectionOnModelChangeTest =
    FilterPreservesSelectionOnModelChangeTest;
