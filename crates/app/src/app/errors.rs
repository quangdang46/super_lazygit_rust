// Ported from ./references/lazygit-master/pkg/app/errors.go

use crate::i18n::TranslationSet;

struct ErrorMapping {
    original_error: String,
    new_error: String,
}

const MIN_GIT_VERSION: &str = "2.32.0";

fn min_git_version_error_message(tr: &TranslationSet) -> String {
    tr.min_git_version_error
        .replace("{{.version}}", MIN_GIT_VERSION)
}

pub fn known_error(tr: &TranslationSet, err: &(dyn std::error::Error)) -> (String, bool) {
    let error_message = err.to_string();

    let known_error_messages = vec![min_git_version_error_message(tr)];

    if known_error_messages.contains(&error_message) {
        return (error_message, true);
    }

    let mappings = vec![
        ErrorMapping {
            original_error: "fatal: not a git repository".to_string(),
            new_error: tr.not_a_repository.clone(),
        },
        ErrorMapping {
            original_error: "getwd: no such file or directory".to_string(),
            new_error: tr.working_directory_does_not_exist.clone(),
        },
    ];

    for mapping in mappings {
        if error_message.contains(&mapping.original_error) {
            return (mapping.new_error, true);
        }
    }

    (String::new(), false)
}
