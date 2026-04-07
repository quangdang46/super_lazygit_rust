use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct ApplyInReverseWithConflictTest;

impl ApplyInReverseWithConflictTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Apply a custom patch in reverse, resulting in a conflict".to_string(),
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

pub const APPLY_IN_REVERSE_WITH_CONFLICT: ApplyInReverseWithConflictTest =
    ApplyInReverseWithConflictTest;
