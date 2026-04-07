use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct DropCommitInCopiedBranchWithUpdateRefTest;

impl DropCommitInCopiedBranchWithUpdateRefTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Drops a commit in a branch that is a copy of another branch, and verify that the other branch is left alone".to_string(),
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

pub const DROP_COMMIT_IN_COPIED_BRANCH_WITH_UPDATE_REF: DropCommitInCopiedBranchWithUpdateRefTest =
    DropCommitInCopiedBranchWithUpdateRefTest;
