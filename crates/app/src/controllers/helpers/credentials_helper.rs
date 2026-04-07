// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/credentials_helper.go

pub struct CredentialsHelper {
    context: HelperCommon,
}

pub struct HelperCommon;

pub enum CredentialType {
    Username,
    Password,
    Passphrase,
    Pin,
    Token,
}

impl CredentialsHelper {
    pub fn new(context: HelperCommon) -> Self {
        Self { context }
    }

    pub fn prompt_user_for_credential(&self, _pass_or_uname: CredentialType) -> String {
        String::new()
    }

    fn get_title_and_mask(&self, _pass_or_uname: CredentialType) -> (String, bool) {
        (String::new(), false)
    }
}
