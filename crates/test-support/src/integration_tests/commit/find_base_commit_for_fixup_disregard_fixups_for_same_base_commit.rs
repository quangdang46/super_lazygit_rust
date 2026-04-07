use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct FindBaseCommitForFixupDisregardFixupsForSameBaseCommitTest;

impl FindBaseCommitForFixupDisregardFixupsForSameBaseCommitTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Finds the base commit to create a fixup for, disregarding fixup commits for the same base commit".to_string(),
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

pub const FIND_BASE_COMMIT_FOR_FIXUP_DISREGARD_FIXUPS_FOR_SAME_BASE_COMMIT:
    FindBaseCommitForFixupDisregardFixupsForSameBaseCommitTest =
    FindBaseCommitForFixupDisregardFixupsForSameBaseCommitTest;
