use std::{
    io::{self, Write},
    sync::{Arc, Mutex, MutexGuard},
};

/// Cloneable writer that serializes each `Write::write` call across all clones.
///
/// The implementation completes the entire supplied buffer while holding the
/// mutex, so callers can make a framed packet one atomic write operation even
/// when a connection loop and a dedicated output worker share one socket.
#[derive(Debug)]
pub struct SharedWriter<W> {
    inner: Arc<Mutex<W>>,
}

impl<W> SharedWriter<W> {
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self {
            inner: Arc::new(Mutex::new(writer)),
        }
    }

    pub fn with_writer<R>(&self, inspect: impl FnOnce(&W) -> R) -> io::Result<R> {
        let writer = lock_writer(&self.inner)?;
        Ok(inspect(&writer))
    }
}

impl<W> Clone for SharedWriter<W> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<W: Write> Write for SharedWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut writer = lock_writer(&self.inner)?;
        writer.write_all(buffer)?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        lock_writer(&self.inner)?.flush()
    }
}

fn lock_writer<W>(writer: &Arc<Mutex<W>>) -> io::Result<MutexGuard<'_, W>> {
    writer
        .lock()
        .map_err(|_| io::Error::other("shared writer mutex is poisoned"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, Barrier, Mutex},
        thread,
    };

    #[derive(Clone, Default)]
    struct SlowRecordingWriter {
        bytes: Arc<Mutex<Vec<u8>>>,
    }

    impl Write for SlowRecordingWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            let mut bytes = self.bytes.lock().unwrap();
            for byte in buffer {
                bytes.push(*byte);
                thread::yield_now();
            }
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn clones_serialize_complete_buffers() {
        let recording = SlowRecordingWriter::default();
        let bytes = Arc::clone(&recording.bytes);
        let mut first = SharedWriter::new(recording);
        let mut second = first.clone();
        let start = Arc::new(Barrier::new(3));

        let first_start = Arc::clone(&start);
        let first_thread = thread::spawn(move || {
            first_start.wait();
            first.write_all(&[1; 1_024]).unwrap();
        });
        let second_start = Arc::clone(&start);
        let second_thread = thread::spawn(move || {
            second_start.wait();
            second.write_all(&[2; 1_024]).unwrap();
        });
        start.wait();
        first_thread.join().unwrap();
        second_thread.join().unwrap();

        let bytes = bytes.lock().unwrap();
        assert_eq!(bytes.len(), 2_048);
        let transition_count = bytes.windows(2).filter(|pair| pair[0] != pair[1]).count();
        assert_eq!(transition_count, 1);
        assert!(bytes.iter().all(|byte| matches!(*byte, 1 | 2)));
    }

    #[test]
    fn exposes_read_only_inspection_under_the_same_lock() {
        let writer = SharedWriter::new(Vec::<u8>::from([1, 2, 3]));
        assert_eq!(writer.with_writer(Vec::len).unwrap(), 3);
    }
}
