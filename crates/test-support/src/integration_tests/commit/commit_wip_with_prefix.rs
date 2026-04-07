use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct CommitWipWithPrefixTest;

impl CommitWipWithPrefixTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Commit with skip hook and config commitPrefix is defined. Prefix is ignored when creating WIP commits.".to_string(),
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

pub const COMMIT_WIP_WITH_PREFIX: CommitWipWithPrefixTest = CommitWipWithPrefixTest;
