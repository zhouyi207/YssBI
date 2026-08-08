use super::{CancellationToken, RunDeadline, RunError, RunPhase, check_terminal};
use crate::node_system::protocol::Value;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

struct SpillFile {
    path: Mutex<PathBuf>,
    promotion: Mutex<()>,
    delete_on_drop: AtomicBool,
    #[cfg(test)]
    deletion_failures: AtomicUsize,
    #[cfg(test)]
    promotion_checkpoint: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl SpillFile {
    fn delete(&self) -> std::io::Result<()> {
        if !self.delete_on_drop.load(Ordering::Acquire) {
            return Ok(());
        }
        #[cfg(test)]
        if self
            .deletion_failures
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok()
        {
            return Err(std::io::Error::other("injected spill deletion failure"));
        }
        let path = self.path.lock().unwrap_or_else(|error| error.into_inner());
        match fs::remove_file(path.as_path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl Drop for SpillFile {
    fn drop(&mut self) {
        for attempt in 0..2 {
            match self.delete() {
                Ok(()) => return,
                Err(_) if attempt == 0 => std::thread::yield_now(),
                Err(error) => {
                    tauri_plugin_log::log::warn!(
                        target: "yssbi::node_system::runtime::cleanup",
                        "failed to remove durable spill file after retry: {error}"
                    );
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct SpillArtifact {
    file: Arc<SpillFile>,
    bytes: u64,
    count: usize,
    max_record_bytes: u64,
}

impl SpillArtifact {
    pub(crate) fn new(path: PathBuf, metadata: SpillMetadata) -> Self {
        Self {
            file: Arc::new(SpillFile {
                path: Mutex::new(path),
                promotion: Mutex::new(()),
                delete_on_drop: AtomicBool::new(false),
                #[cfg(test)]
                deletion_failures: AtomicUsize::new(0),
                #[cfg(test)]
                promotion_checkpoint: Mutex::new(None),
            }),
            bytes: metadata.bytes,
            count: metadata.count,
            max_record_bytes: metadata.max_record_bytes,
        }
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub const fn len(&self) -> usize {
        self.count
    }

    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn cursor(&self) -> Result<SpillCursor, RunError> {
        SpillCursor::open(Arc::clone(&self.file), self.max_record_bytes)
    }

    #[cfg(test)]
    pub(crate) fn path_for_test(&self) -> PathBuf {
        self.file
            .path
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    #[cfg(test)]
    pub(crate) fn fail_next_deletions_for_test(&self, count: usize) {
        self.file.deletion_failures.store(count, Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) fn set_promotion_checkpoint_for_test(
        &self,
        checkpoint: Arc<dyn Fn() + Send + Sync>,
    ) {
        *self
            .file
            .promotion_checkpoint
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(checkpoint);
    }

    pub(crate) fn promote(
        &self,
        cancellation: &CancellationToken,
        deadline: Option<RunDeadline>,
    ) -> Result<(), RunError> {
        check_terminal(cancellation, deadline, RunPhase::ResultPublication)?;
        let _promotion = self
            .file
            .promotion
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if self.file.delete_on_drop.load(Ordering::Acquire) {
            return Ok(());
        }
        let source = self
            .file
            .path
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let durable_root = std::env::temp_dir().join("yssbi-runtime-results");
        fs::create_dir_all(&durable_root).map_err(spill_io_error)?;
        let durable = durable_root.join(format!("result-{}.jsonf", uuid::Uuid::new_v4()));
        fs::rename(&source, &durable).map_err(spill_io_error)?;
        #[cfg(test)]
        if let Some(checkpoint) = self
            .file
            .promotion_checkpoint
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
        {
            checkpoint();
        }
        let mut path = self
            .file
            .path
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Err(error) = check_terminal(cancellation, deadline, RunPhase::ResultPublication) {
            drop(path);
            let _ = fs::remove_file(&durable);
            return Err(error);
        }
        *path = durable;
        self.file.delete_on_drop.store(true, Ordering::Release);
        Ok(())
    }
}

impl std::fmt::Debug for SpillArtifact {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SpillArtifact")
            .field("bytes", &self.bytes)
            .field("count", &self.count)
            .finish_non_exhaustive()
    }
}

impl PartialEq for SpillArtifact {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.file, &other.file)
    }
}

impl Eq for SpillArtifact {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayArtifact {
    spill: SpillArtifact,
}

impl ReplayArtifact {
    pub(crate) fn new(spill: SpillArtifact) -> Self {
        Self { spill }
    }

    pub fn cursor(&self) -> Result<SpillCursor, RunError> {
        self.spill.cursor()
    }

    pub const fn len(&self) -> usize {
        self.spill.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.spill.is_empty()
    }

    pub(crate) fn promote(
        &self,
        cancellation: &CancellationToken,
        deadline: Option<RunDeadline>,
    ) -> Result<(), RunError> {
        self.spill.promote(cancellation, deadline)
    }

    pub(crate) fn spill(&self) -> &SpillArtifact {
        &self.spill
    }
}

pub struct SpillCursor {
    reader: Option<BufReader<File>>,
    file: Arc<SpillFile>,
    finished: bool,
    max_record_bytes: u64,
}

impl SpillCursor {
    fn open(file: Arc<SpillFile>, max_record_bytes: u64) -> Result<Self, RunError> {
        let path = file
            .path
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let reader = File::open(path)
            .map(BufReader::new)
            .map_err(spill_io_error)?;
        Ok(Self {
            reader: Some(reader),
            file,
            finished: false,
            max_record_bytes,
        })
    }

    pub fn close(&mut self) -> Result<(), RunError> {
        self.reader.take();
        if Arc::strong_count(&self.file) == 1 {
            self.file.delete().map_err(spill_delete_error)?;
        }
        Ok(())
    }

    fn reader(&mut self) -> &mut BufReader<File> {
        self.reader
            .as_mut()
            .expect("spill cursor reader exists until close or drop")
    }
}

impl Drop for SpillCursor {
    fn drop(&mut self) {
        self.reader.take();
        if Arc::strong_count(&self.file) == 1
            && let Err(error) = self.file.delete()
        {
            tauri_plugin_log::log::warn!(
                target: "yssbi::node_system::runtime::cleanup",
                "failed to remove durable spill file after cursor close: {error}"
            );
        }
    }
}

impl Iterator for SpillCursor {
    type Item = Result<Value, RunError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let mut length = [0_u8; 8];
        match self.reader().read_exact(&mut length) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                self.finished = true;
                return None;
            }
            Err(error) => {
                self.finished = true;
                return Some(Err(spill_io_error(error)));
            }
        }
        let length = u64::from_le_bytes(length);
        if length > self.max_record_bytes {
            self.finished = true;
            return Some(Err(RunError::Stream(
                "spill record exceeds completed artifact metadata".into(),
            )));
        }
        let Ok(length) = usize::try_from(length) else {
            self.finished = true;
            return Some(Err(RunError::Stream(
                "spill record length exceeds this platform".into(),
            )));
        };
        let mut encoded = vec![0; length];
        if let Err(error) = self.reader().read_exact(&mut encoded) {
            self.finished = true;
            return Some(Err(spill_io_error(error)));
        }
        Some(
            serde_json::from_slice(&encoded)
                .map_err(|error| RunError::Stream(format!("invalid spill value: {error}").into())),
        )
    }
}

pub(crate) fn serialized_value_len(value: &Value) -> Result<u64, RunError> {
    let mut counter = CountingWriter(0);
    serde_json::to_writer(&mut counter, value).map_err(|error| {
        RunError::Stream(format!("failed to measure serialized value: {error}").into())
    })?;
    Ok(counter.0)
}

struct CountingWriter(u64);

impl Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_add(buffer.len() as u64)
            .ok_or_else(|| std::io::Error::other("serialized value size overflowed"))?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SpillMetadata {
    pub bytes: u64,
    pub count: usize,
    pub max_record_bytes: u64,
}

pub(crate) fn write_spill(
    path: &Path,
    values: impl Iterator<Item = Result<Value, RunError>>,
    cancellation: &CancellationToken,
    mut reserve: impl FnMut(u64) -> Result<(), RunError>,
) -> Result<SpillMetadata, RunError> {
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(spill_io_error)?;
    let mut writer = BufWriter::new(file);
    let mut written = 0_u64;
    let mut count = 0_usize;
    let mut max_record_bytes = 0_u64;
    for value in values {
        cancellation.check()?;
        let value = value?;
        let encoded_bytes = serialized_value_len(&value)?;
        let record_bytes = 8_u64
            .checked_add(encoded_bytes)
            .ok_or_else(|| RunError::Stream("spill record size overflowed".into()))?;
        reserve(record_bytes)?;
        writer
            .write_all(&encoded_bytes.to_le_bytes())
            .map_err(spill_io_error)?;
        serde_json::to_writer(&mut writer, &value).map_err(|error| {
            RunError::Stream(format!("failed to serialize spill value: {error}").into())
        })?;
        written = written
            .checked_add(record_bytes)
            .ok_or_else(|| RunError::Stream("spill size overflowed".into()))?;
        count = count
            .checked_add(1)
            .ok_or_else(|| RunError::Stream("spill value count overflowed".into()))?;
        max_record_bytes = max_record_bytes.max(encoded_bytes);
    }
    writer.flush().map_err(spill_io_error)?;
    writer.get_ref().sync_all().map_err(spill_io_error)?;
    Ok(SpillMetadata {
        bytes: written,
        count,
        max_record_bytes,
    })
}

fn spill_io_error(error: std::io::Error) -> RunError {
    RunError::Stream(format!("spill I/O failed: {error}").into())
}

fn spill_delete_error(error: std::io::Error) -> RunError {
    RunError::Stream(format!("spill deletion failed: {error}").into())
}
