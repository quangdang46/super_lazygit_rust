use std::io::{self, Write};
use std::sync::Mutex;

pub struct OnceWriter<W: Write> {
    writer: W,
    initialized: Mutex<bool>,
    init_fn: Mutex<Option<Box<dyn FnOnce() + Send + Sync>>>,
}

impl<W: Write> OnceWriter<W> {
    pub fn new<F>(writer: W, init_fn: F) -> Self
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        Self {
            writer,
            initialized: Mutex::new(false),
            init_fn: Mutex::new(Some(Box::new(init_fn))),
        }
    }

    fn ensure_initialized(&self) {
        let mut initialized = self.initialized.lock().unwrap();
        if !*initialized {
            if let Some(f) = self.init_fn.lock().unwrap().take() {
                f();
            }
            *initialized = true;
        }
    }
}

impl<W: Write> Write for OnceWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.ensure_initialized();
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_once_writer_calls_function_once() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        let mut writer = OnceWriter::new(Cursor::new(Vec::new()), move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        writer.write_all(b"hello").unwrap();
        writer.write_all(b" world").unwrap();
        writer.write_all(b"!").unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_once_writer_writes_content() {
        let mut buffer = Vec::new();
        let mut writer = OnceWriter::new(&mut buffer, || {});

        writer.write_all(b"hello").unwrap();
        writer.flush().unwrap();

        assert_eq!(buffer, b"hello");
    }
}
