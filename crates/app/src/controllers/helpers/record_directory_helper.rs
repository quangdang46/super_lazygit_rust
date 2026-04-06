// Ported from ./references/lazygit-master/pkg/gui/controllers/helpers/record_directory_helper.go

use std::env;
use std::fs;
use std::path::Path;

pub struct RecordDirectoryHelper {
    common: HelperCommon,
}

pub struct HelperCommon;

impl RecordDirectoryHelper {
    pub fn new(common: HelperCommon) -> Self {
        Self { common }
    }

    pub fn record_current_directory(&self) -> Result<(), String> {
        let dir_name = env::current_dir().map_err(|e| e.to_string())?;
        self.record_directory(dir_name)
    }

    pub fn record_directory<P: AsRef<Path>>(&self, dir_name: P) -> Result<(), String> {
        let new_dir_file_path = match env::var("LAZYGIT_NEW_DIR_FILE") {
            Ok(path) if !path.is_empty() => path,
            _ => return Ok(()),
        };

        fs::write(
            &new_dir_file_path,
            dir_name.as_ref().to_string_lossy().as_bytes(),
        )
        .map_err(|e| e.to_string())
    }
}
