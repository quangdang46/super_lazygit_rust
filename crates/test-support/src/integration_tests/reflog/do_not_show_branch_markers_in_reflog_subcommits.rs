use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct DoNotShowBranchMarkersInReflogSubcommitsTest;

impl DoNotShowBranchMarkersInReflogSubcommitsTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description:
                "Verify that no branch heads are shown in the subcommits view of a reflog entry"
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

pub const DO_NOT_SHOW_BRANCH_MARKERS_IN_REFLOG_SUB_COMMITS:
    DoNotShowBranchMarkersInReflogSubcommitsTest = DoNotShowBranchMarkersInReflogSubcommitsTest;
