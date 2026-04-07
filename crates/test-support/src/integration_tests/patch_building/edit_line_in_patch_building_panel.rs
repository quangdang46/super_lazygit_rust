use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct EditLineInPatchBuildingPanelTest;

impl EditLineInPatchBuildingPanelTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description:
                "Edit a line in the patch building panel; make sure we end up on the right line"
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

pub const EDIT_LINE_IN_PATCH_BUILDING_PANEL: EditLineInPatchBuildingPanelTest =
    EditLineInPatchBuildingPanelTest;
