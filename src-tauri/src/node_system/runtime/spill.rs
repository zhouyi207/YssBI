use super::{
    ArtifactValueKind, CancellationToken, DataSeriesMetadata, RunDeadline, RunError, RunPhase,
    check_terminal,
};
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
    promoted: AtomicBool,
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
                    tracing::warn!(
                        target: "yssbi::node_system::runtime::cleanup",
                        diagnostic_domain = "execution",
                        diagnostic_event = "spillDeleteFailed",
                        error = %error,
                        "Failed to remove durable spill file after retry"
                    );
                }
            }
        }
    }
}

pub(crate) struct SpillStorage {
    file: Arc<SpillFile>,
    bytes: u64,
    count: usize,
    max_record_bytes: u64,
    value_kind: ArtifactValueKind,
    data_series_metadata: Option<DataSeriesMetadata>,
    logical_digest: [u8; 32],
    _reservation: Option<super::materialization::SpillReservation>,
}

impl SpillStorage {
    pub(crate) fn new(
        path: PathBuf,
        metadata: SpillMetadata,
        value_kind: ArtifactValueKind,
        data_series_metadata: Option<DataSeriesMetadata>,
        logical_digest: [u8; 32],
        reservation: Option<super::materialization::SpillReservation>,
    ) -> Self {
        Self {
            file: Arc::new(SpillFile {
                path: Mutex::new(path),
                promotion: Mutex::new(()),
                delete_on_drop: AtomicBool::new(true),
                promoted: AtomicBool::new(false),
                #[cfg(test)]
                deletion_failures: AtomicUsize::new(0),
                #[cfg(test)]
                promotion_checkpoint: Mutex::new(None),
            }),
            bytes: metadata.bytes,
            count: metadata.count,
            max_record_bytes: metadata.max_record_bytes,
            value_kind,
            data_series_metadata,
            logical_digest,
            _reservation: reservation,
        }
    }

    pub(crate) const fn value_kind(&self) -> ArtifactValueKind {
        self.value_kind
    }

    pub(crate) fn data_series_metadata(&self) -> Option<&DataSeriesMetadata> {
        self.data_series_metadata.as_ref()
    }

    pub(crate) const fn logical_digest(&self) -> [u8; 32] {
        self.logical_digest
    }

    pub(crate) fn has_reservation(&self) -> bool {
        self._reservation.is_some()
    }

    pub const fn len(&self) -> usize {
        self.count
    }

    pub fn cursor(&self) -> Result<SpillCursor, RunError> {
        SpillCursor::open(Arc::clone(&self.file), self.max_record_bytes, self.count)
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
        if self.file.promoted.load(Ordering::Acquire) {
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
        self.file.promoted.store(true, Ordering::Release);
        Ok(())
    }
}

impl std::fmt::Debug for SpillStorage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SpillStorage")
            .field("bytes", &self.bytes)
            .field("count", &self.count)
            .finish_non_exhaustive()
    }
}

impl PartialEq for SpillStorage {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.file, &other.file)
    }
}

impl Eq for SpillStorage {}

pub(crate) struct SpillCursor {
    reader: Option<BufReader<File>>,
    file: Arc<SpillFile>,
    finished: bool,
    max_record_bytes: u64,
    expected_count: usize,
    read_count: usize,
}

impl SpillCursor {
    fn open(
        file: Arc<SpillFile>,
        max_record_bytes: u64,
        expected_count: usize,
    ) -> Result<Self, RunError> {
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
            expected_count,
            read_count: 0,
        })
    }

    #[cfg(test)]
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
            tracing::warn!(
                target: "yssbi::node_system::runtime::cleanup",
                diagnostic_domain = "execution",
                diagnostic_event = "spillDeleteFailed",
                error = %error,
                "Failed to remove durable spill file after cursor close"
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
        if self.read_count == self.expected_count {
            let mut trailing = [0_u8; 1];
            return match self.reader().read(&mut trailing) {
                Ok(0) => {
                    self.finished = true;
                    None
                }
                Ok(_) => {
                    self.finished = true;
                    Some(Err(RunError::Stream(
                        "spill contains trailing data after declared value count".into(),
                    )))
                }
                Err(error) => {
                    self.finished = true;
                    Some(Err(spill_io_error(error)))
                }
            };
        }

        let mut length = [0_u8; 8];
        match self.reader().read(&mut length[..1]) {
            Ok(0) => {
                self.finished = true;
                return Some(Err(RunError::Stream(
                    "spill ended before declared value count".into(),
                )));
            }
            Ok(1) => {}
            Ok(_) => unreachable!("one-byte read returned more than one byte"),
            Err(error) => {
                self.finished = true;
                return Some(Err(spill_io_error(error)));
            }
        }
        if let Err(error) = self.reader().read_exact(&mut length[1..]) {
            self.finished = true;
            return Some(Err(if error.kind() == std::io::ErrorKind::UnexpectedEof {
                RunError::Stream("truncated spill length prefix".into())
            } else {
                spill_io_error(error)
            }));
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
            return Some(Err(if error.kind() == std::io::ErrorKind::UnexpectedEof {
                RunError::Stream("truncated spill record payload".into())
            } else {
                spill_io_error(error)
            }));
        }
        self.read_count += 1;
        Some(
            serde_json::from_slice(&encoded)
                .map_err(|error| RunError::Stream(format!("invalid spill value: {error}").into())),
        )
    }
}

pub(crate) fn append_spill_value(
    path: &Path,
    value: &Value,
    cancellation: &CancellationToken,
    deadline: Option<RunDeadline>,
) -> Result<u64, RunError> {
    check_terminal(cancellation, deadline, RunPhase::AdapterIo)?;
    let encoded_bytes = serialized_value_len(value)?;
    let mut writer = BufWriter::new(
        OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(spill_io_error)?,
    );
    writer
        .write_all(&encoded_bytes.to_le_bytes())
        .map_err(spill_io_error)?;
    serde_json::to_writer(&mut writer, value).map_err(|error| {
        RunError::Stream(format!("failed to serialize spill value: {error}").into())
    })?;
    writer.flush().map_err(spill_io_error)?;
    check_terminal(cancellation, deadline, RunPhase::AdapterIo)?;
    writer.get_ref().sync_all().map_err(spill_io_error)?;
    check_terminal(cancellation, deadline, RunPhase::AdapterIo)?;
    Ok(encoded_bytes)
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

#[cfg(test)]
fn spill_delete_error(error: std::io::Error) -> RunError {
    RunError::Stream(format!("spill deletion failed: {error}").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_storage(bytes: &[u8], count: usize, max_record_bytes: u64) -> (SpillStorage, PathBuf) {
        let path = std::env::temp_dir().join(format!("spill-frame-{}", uuid::Uuid::new_v4()));
        fs::write(&path, bytes).unwrap();
        let storage = SpillStorage::new(
            path.clone(),
            SpillMetadata {
                bytes: bytes.len() as u64,
                count,
                max_record_bytes,
            },
            ArtifactValueKind::Sequence,
            None,
            [0; 32],
            None,
        );
        (storage, path)
    }

    #[test]
    fn spill_cursor_rejects_truncated_length_prefix() {
        let (storage, _) = test_storage(&[1, 0, 0], 1, 1);
        let error = storage.cursor().unwrap().next().unwrap().unwrap_err();
        assert!(error.to_string().contains("truncated spill length prefix"));
    }

    #[test]
    fn spill_cursor_rejects_early_eof_before_declared_count() {
        let (storage, _) = test_storage(&[], 1, 1);
        let error = storage.cursor().unwrap().next().unwrap().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("ended before declared value count")
        );
    }

    #[test]
    fn spill_cursor_rejects_trailing_records_after_declared_count() {
        let value = serde_json::to_vec(&Value::Integer(1)).unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&value);
        let (storage, _) = test_storage(&bytes, 0, value.len() as u64);
        let error = storage.cursor().unwrap().next().unwrap().unwrap_err();
        assert!(error.to_string().contains("trailing data"));
    }

    #[test]
    fn spill_cursor_rejects_truncated_record_payload() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&5_u64.to_le_bytes());
        bytes.extend_from_slice(b"12");
        let (storage, _) = test_storage(&bytes, 1, 5);
        let error = storage.cursor().unwrap().next().unwrap().unwrap_err();
        assert!(error.to_string().contains("truncated spill record payload"));
    }
}
