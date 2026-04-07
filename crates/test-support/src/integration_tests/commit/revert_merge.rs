use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct RevertMergeTest;

impl RevertMergeTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Reverts a merge commit and chooses to revert to the parent commit"
                .to_string(),
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

pub const REVERT_MERGE: RevertMergeTest = RevertMergeTest;
