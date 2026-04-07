use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct StageAllStagesOnlyTrackedFilesInTrackedOnlyFilterTest;

impl StageAllStagesOnlyTrackedFilesInTrackedOnlyFilterTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Staging all files in tracked only view should stage only tracked files"
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

pub const STAGE_ALL_STAGES_ONLY_TRACKED_FILES_IN_TRACKED_ONLY_FILTER:
    StageAllStagesOnlyTrackedFilesInTrackedOnlyFilterTest =
    StageAllStagesOnlyTrackedFilesInTrackedOnlyFilterTest;
