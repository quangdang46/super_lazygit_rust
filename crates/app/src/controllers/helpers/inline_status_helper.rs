// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/inline_status_helper.go

pub struct InlineStatusHelper {
    context: HelperCommon,
    window_helper: WindowHelper,
    contexts_with_inline_status: Vec<(String, InlineStatusInfo)>,
}

pub struct HelperCommon;

pub struct WindowHelper;

pub struct InlineStatusOpts {
    pub item: String,
    pub operation: ItemOperation,
    pub context_key: String,
}

pub struct InlineStatusInfo {
    pub ref_count: i64,
    pub stop: bool,
}

pub enum ItemOperation {
    None,
    Pushing,
    Pulling,
    FastForwarding,
    Deleting,
    Fetching,
    CheckingOut,
}

impl InlineStatusHelper {
    pub fn new(context: HelperCommon, window_helper: WindowHelper) -> Self {
        Self {
            context,
            window_helper,
            contexts_with_inline_status: Vec::new(),
        }
    }

    pub fn with_inline_status(
        &self,
        _opts: InlineStatusOpts,
        _f: fn() -> Result<(), String>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn start(&self, _opts: InlineStatusOpts) {}

    fn stop(&self, _opts: InlineStatusOpts) {}

    fn render_context(&self, _context_key: &str) {}
}

impl InlineStatusHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
            window_helper: WindowHelper,
            contexts_with_inline_status: Vec::new(),
        }
    }
}

impl Default for InlineStatusHelper {
    fn default() -> Self {
        Self::new()
    }
}
