use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

/// Iterates over each line in a file, calling the provided function for each line.
pub fn for_each_line_in_file<F>(path: &Path, mut f: F) -> io::Result<()>
where
    F: FnMut(String, usize),
{
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    for_each_line_in_stream(reader, |line, i| f(line, i));
    Ok(())
}

fn for_each_line_in_stream<R: BufRead, F>(reader: R, mut f: F)
where
    F: FnMut(String, usize),
{
    for (i, line_result) in reader.lines().enumerate() {
        if let Ok(line) = line_result {
            f(line, i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_for_each_line_in_file() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("test.txt");
        std::fs::write(&path, "line1\nline2\nline3\n").unwrap();

        let mut lines = Vec::new();
        for_each_line_in_file(&path, |line, _| {
            lines.push(line);
        })
        .unwrap();

        assert_eq!(lines, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn test_for_each_line_in_stream() {
        let cursor = Cursor::new("a\nb\nc\n");
        let mut lines = Vec::new();
        for_each_line_in_stream(cursor, |line, _| {
            lines.push(line);
        });
        assert_eq!(lines, vec!["a", "b", "c"]);
    }
}
