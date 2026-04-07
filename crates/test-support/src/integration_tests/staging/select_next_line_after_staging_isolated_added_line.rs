use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct SelectNextLineAfterStagingIsolatedAddedLineTest;

impl SelectNextLineAfterStagingIsolatedAddedLineTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description:
                "After staging an isolated added line, the cursor advances to the next hunk"
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

pub const SELECT_NEXT_LINE_AFTER_STAGING_ISOLATED_ADDED_LINE:
    SelectNextLineAfterStagingIsolatedAddedLineTest =
    SelectNextLineAfterStagingIsolatedAddedLineTest;
