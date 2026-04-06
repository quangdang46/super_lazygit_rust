use crate::integration_test::{IntegrationTest, NewIntegrationTestArgs};

pub struct DiffContextChangeTest;

impl DiffContextChangeTest {
    pub fn new() -> IntegrationTest {
        IntegrationTest::new(NewIntegrationTestArgs {
            description: "Change the number of diff context lines while in the staging panel"
                .to_string(),
            extra_cmd_args: vec![],
            skip: false,
            setup_repo: Some(Box::new(|shell| {
                shell.create_file_and_add(
                    "file1",
                    "1a\n2a\n3a\n4a\n5a\n6a\n7a\n8a\n9a\n10a\n11a\n12a\n13a\n14a\n15a",
                );
                shell.git_add(".");
                shell.update_file(
                    "file1",
                    "1a\n2a\n3b\n4a\n5a\n6a\n7a\n8a\n9a\n10a\n11a\n12a\n13b\n14a\n15a",
                );
            })),
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

pub const DIFF_CONTEXT_CHANGE: DiffContextChangeTest = DiffContextChangeTest;
