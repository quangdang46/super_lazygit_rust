use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct MoveToIndexPartOfAdjacentAddedLinesTest;

impl MoveToIndexPartOfAdjacentAddedLinesTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Move a patch from a commit to the index, with only some lines of a range of adjacent added lines in the patch".to_string(),
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

pub const MOVE_TO_INDEX_PART_OF_ADJACENT_ADDED_LINES: MoveToIndexPartOfAdjacentAddedLinesTest =
    MoveToIndexPartOfAdjacentAddedLinesTest;
