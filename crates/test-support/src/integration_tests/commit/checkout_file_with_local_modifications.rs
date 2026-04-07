use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct CheckoutFileWithLocalModificationsTest;

impl CheckoutFileWithLocalModificationsTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Checkout a file from a commit that has local modifications".to_string(),
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

pub const CHECKOUT_FILE_WITH_LOCAL_MODIFICATIONS: CheckoutFileWithLocalModificationsTest =
    CheckoutFileWithLocalModificationsTest;
