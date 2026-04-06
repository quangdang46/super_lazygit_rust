use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct DiffNonStickyRangeTest;

impl DiffNonStickyRangeTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "View the combined diff of multiple commits using a range selection"
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

pub const DIFF_NON_STICKY_RANGE: DiffNonStickyRangeTest = DiffNonStickyRangeTest;
