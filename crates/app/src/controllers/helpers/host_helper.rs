// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/host_helper.go

pub struct HostHelper {
    context: HelperCommon,
}

pub struct HelperCommon;

impl HostHelper {
    pub fn new(context: HelperCommon) -> Self {
        Self { context }
    }

    pub fn get_pull_request_url(&self, _from: &str, _to: &str) -> Result<String, String> {
        Ok(String::new())
    }

    pub fn get_commit_url(&self, _commit_hash: &str) -> Result<String, String> {
        Ok(String::new())
    }

    fn get_hosting_service_mgr(&self) -> Result<HostingServiceMgr, String> {
        Ok(HostingServiceMgr)
    }
}

pub struct HostingServiceMgr;
