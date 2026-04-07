use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct MoveToIndexWorksEvenIfNoprefixIsSetTest;

impl MoveToIndexWorksEvenIfNoprefixIsSetTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description:
                "Moving a patch to the index works even if diff.noprefix or diff.external are set"
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

pub const MOVE_TO_INDEX_WORKS_EVEN_IF_NOPREFIX_IS_SET: MoveToIndexWorksEvenIfNoprefixIsSetTest =
    MoveToIndexWorksEvenIfNoprefixIsSetTest;
