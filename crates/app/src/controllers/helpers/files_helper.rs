// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/files_helper.go

pub struct FilesHelper {
    context: HelperCommon,
}

pub struct HelperCommon;

impl FilesHelper {
    pub fn new(context: HelperCommon) -> Self {
        Self { context }
    }

    pub fn edit_files(&self, _filenames: &[String]) -> Result<(), String> {
        Ok(())
    }

    pub fn edit_file_at_line(&self, _filename: &str, _line_number: i64) -> Result<(), String> {
        Ok(())
    }

    pub fn edit_file_at_line_and_wait(
        &self,
        _filename: &str,
        _line_number: i64,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn open_dir_in_editor(&self, _path: &str) -> Result<(), String> {
        Ok(())
    }

    fn call_editor(&self, _cmd_str: &str, _suspend: bool) -> Result<(), String> {
        Ok(())
    }

    pub fn open_file(&self, _filename: &str) -> Result<(), String> {
        Ok(())
    }
}
