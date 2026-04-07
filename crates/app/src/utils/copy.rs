/* MIT License
 *
 * Copyright (c) 2017 Roland Singer [roland.singer@desertbit.com]
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

use std::fs;
use std::io;
use std::path::Path;

/// CopyFile copies the contents of the file named src to the file named
/// by dst. The file will be created if it does not already exist. If the
/// destination file exists, all its contents will be replaced by the contents
/// of the source file. The file mode will be copied from the source and
/// the copied data is synced/flushed to stable storage.
///
/// # Errors
///
/// Returns an error if the source file cannot be read, if the destination
/// file cannot be created, or if the sync operation fails.
pub fn copy_file(src: &Path, dst: &Path) -> io::Result<()> {
    // Open source file
    let mut in_file = fs::File::open(src)?;
    let metadata = in_file.metadata()?;

    // Create destination file
    let mut out_file = fs::File::create(dst)?;

    // Copy contents
    io::copy(&mut in_file, &mut out_file)?;

    // Sync to storage
    out_file.sync_all()?;

    // Set permissions from source
    fs::set_permissions(dst, metadata.permissions())?;

    Ok(())
}

/// CopyDir recursively copies a directory tree, attempting to preserve permissions.
/// Source directory must exist. If destination already exists it will be overwritten.
/// Symlinks are ignored and skipped.
///
/// # Errors
///
/// Returns an error if the source is not a directory, if destination removal fails,
/// if directory creation fails, or if file copying fails.
pub fn copy_dir(src: &Path, dst: &Path) -> io::Result<()> {
    let src = src.canonicalize()?;
    let dst = dst.canonicalize()?;

    // Check source is a directory
    if !src.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source is not a directory",
        ));
    }

    // Create destination directory
    if dst.exists() {
        fs::remove_dir_all(&dst)?;
    }

    let src_metadata = src.metadata()?;
    fs::create_dir_all(&dst)?;

    // Set permissions
    #[cfg(unix)]
    fs::set_permissions(&dst, src_metadata.permissions())?;

    // Read and copy entries
    for entry in fs::read_dir(&src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let entry_metadata = entry.metadata()?;

        if entry_path.is_dir() {
            copy_dir(&entry_path, &dst_path)?;
        } else if entry_metadata.file_type().is_symlink() {
            // Skip symlinks
            continue;
        } else {
            copy_file(&entry_path, &dst_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_copy_file() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source.txt");
        let dst = dir.path().join("dest.txt");

        // Create source file
        {
            let mut file = fs::File::create(&src).unwrap();
            file.write_all(b"hello world").unwrap();
        }

        // Copy file
        copy_file(&src, &dst).unwrap();

        // Verify contents
        let contents = fs::read_to_string(&dst).unwrap();
        assert_eq!(contents, "hello world");
    }

    #[test]
    fn test_copy_dir() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source");
        let dst = dir.path().join("dest");

        // Create source directory structure
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("file1.txt"), "content1").unwrap();
        fs::create_dir_all(src.join("subdir")).unwrap();
        fs::write(src.join("subdir").join("file2.txt"), "content2").unwrap();

        // Copy directory
        copy_dir(&src, &dst).unwrap();

        // Verify structure
        assert!(dst.is_dir());
        assert!(dst.join("file1.txt").is_file());
        assert!(dst.join("subdir").is_dir());
        assert!(dst.join("subdir").join("file2.txt").is_file());

        // Verify contents
        assert_eq!(fs::read_to_string(dst.join("file1.txt")).unwrap(), "content1");
        assert_eq!(
            fs::read_to_string(dst.join("subdir").join("file2.txt")).unwrap(),
            "content2"
        );
    }

    #[test]
    fn test_copy_dir_nonexistent() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("nonexistent");
        let dst = dir.path().join("dest");

        let result = copy_dir(&src, &dst);
        assert!(result.is_err());
    }
}
