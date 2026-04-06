// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/files_helper.go

pub struct FilesHelper {
    context: HelperCommon,
}

pub struct HelperCommon;

impl FilesHelper {
    pub fn new(context: HelperCommon) -> Self {
        Self { context }
    }

    pub fn edit_files(&self, filenames: &[String]) -> Result<(), String> {
        Ok(())
    }

    pub fn edit_file_at_line(&self, filename: &str, line_number: i64) -> Result<(), String> {
        Ok(())
    }

    pub fn edit_file_at_line_and_wait(
        &self,
        filename: &str,
        line_number: i64,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn open_dir_in_editor(&self, path: &str) -> Result<(), String> {
        Ok(())
    }

    fn call_editor(&self, cmd_str: &str, suspend: bool) -> Result<(), String> {
        Ok(())
    }

    pub fn open_file(&self, filename: &str) -> Result<(), String> {
        Ok(())
    }
}

impl FilesHelper {
    pub fn new() -> Self {
        Self {
            context: HelperCommon,
        }
    }
}

impl Default for FilesHelper {
    fn default() -> Self {
        Self::new()
    }
}
