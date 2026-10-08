use std::future::Future;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TrySendError},
};
use std::time::Duration;

use arrow::array::ArrayBuilder;
use arrow::datatypes::SchemaRef;
use arrow::error::ArrowError;
use arrow::record_batch::{RecordBatch, RecordBatchReader};
use futures_util::{Stream, StreamExt};
use yss_database_contract::DatabaseEngineSql;
use yss_relational_contract::{RelationControl, RelationError};

use crate::SqlSourceError;
use crate::batch::{BatchBuilder, ColumnSpec};

enum Message {
    Schema(SchemaRef),
    Batch(RecordBatch),
    Failed(SqlSourceError),
    Finished,
}

pub struct SqlBatchReader {
    schema: SchemaRef,
    receiver: Option<Receiver<Message>>,
    worker: Option<std::thread::JoinHandle<()>>,
    stop: Arc<AtomicBool>,
    control: RelationControl,
}

pub(crate) struct BatchSender {
    sender: SyncSender<Message>,
    stop: Arc<AtomicBool>,
    control: RelationControl,
}

impl BatchSender {
    pub async fn read_rows<R: sqlx::Row>(
        &self,
        engine: &'static str,
        columns: Vec<ColumnSpec>,
        mut rows: impl Stream<Item = Result<R, sqlx::Error>> + Unpin,
        decode: impl Fn(&R, usize, &ColumnSpec, &mut dyn ArrayBuilder) -> Result<usize, SqlSourceError>,
    ) -> Result<(), SqlSourceError> {
        let mut builder = BatchBuilder::new(columns);
        self.schema(builder.schema.clone()).await?;
        let max_bytes = self.control.max_input_bytes;
        while let Some(row) = rows.next().await {
            let row = row.map_err(|source| SqlSourceError::query(engine, "read table", source))?;
            builder.append_row(&row, &decode)?;
            if builder.bytes > max_bytes {
                return Err(RelationError::MemoryLimitExceeded.into());
            }
            if builder.rows >= 50_000 || builder.bytes >= max_bytes / 2 {
                self.batch(builder.finish()?).await?;
            }
        }
        if builder.rows > 0 {
            self.batch(builder.finish()?).await?;
        }
        Ok(())
    }
    fn check(&self) -> Result<(), SqlSourceError> {
        if self.stop.load(Ordering::Acquire) {
            return Err(SqlSourceError::Closed);
        }
        self.control.check().map_err(Into::into)
    }
    pub async fn controlled<T>(
        &self,
        future: impl Future<Output = T>,
    ) -> Result<T, SqlSourceError> {
        tokio::pin!(future);
        loop {
            self.check()?;
            tokio::select! { value = &mut future => { self.check()?; return Ok(value); }, _ = tokio::time::sleep(Duration::from_millis(20)) => {} }
        }
    }
    async fn send(&self, mut message: Message) -> Result<(), SqlSourceError> {
        loop {
            self.check()?;
            match self.sender.try_send(message) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Disconnected(_)) => return Err(SqlSourceError::Closed),
                Err(TrySendError::Full(pending)) => message = pending,
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
    async fn schema(&self, schema: SchemaRef) -> Result<(), SqlSourceError> {
        self.send(Message::Schema(schema)).await
    }
    async fn batch(&self, batch: RecordBatch) -> Result<(), SqlSourceError> {
        if batch.get_array_memory_size() > self.control.max_input_bytes {
            return Err(RelationError::MemoryLimitExceeded.into());
        }
        self.send(Message::Batch(batch)).await
    }
}

pub(crate) fn read_table(
    engine: &DatabaseEngineSql,
    connection: &str,
    table: &str,
    control: RelationControl,
) -> Result<SqlBatchReader, SqlSourceError> {
    control.check()?;
    if control.max_input_bytes == 0 {
        return Err(RelationError::MemoryLimitExceeded.into());
    }
    let (sender, receiver) = mpsc::sync_channel(2);
    let stop = Arc::new(AtomicBool::new(false));
    let output = BatchSender {
        sender,
        stop: stop.clone(),
        control: control.clone(),
    };
    let engine = engine.clone();
    let connection = connection.to_owned();
    let table = table.to_owned();
    let worker = std::thread::Builder::new()
        .name("yss-sql-batches".into())
        .spawn(move || {
            let failure_sender = output.sender.clone();
            let result = crate::runtime::run(async move {
                let work = async {
                    match engine {
                        DatabaseEngineSql::Sqlite { auto_create } => {
                            crate::sqlite::read_table(&connection, auto_create, &table, &output)
                                .await
                        }
                        DatabaseEngineSql::Postgres { ssl } => {
                            crate::postgres::read_table(&connection, ssl, &table, &output).await
                        }
                        DatabaseEngineSql::Mysql { charset } => {
                            crate::mysql::read_table(&connection, &charset, &table, &output).await
                        }
                    }
                };
                let result = output.controlled(work).await.and_then(|result| result);
                match result {
                    Ok(()) => output.send(Message::Finished).await?,
                    Err(error) => output.send(Message::Failed(error)).await?,
                }
                Ok(())
            });
            // Runtime initialization can fail before an async sender exists.
            if let Err(error) = result {
                let _ = failure_sender.try_send(Message::Failed(error));
            }
        })
        .map_err(SqlSourceError::RuntimeThread)?;
    let mut reader = SqlBatchReader {
        schema: Arc::new(arrow::datatypes::Schema::empty()),
        receiver: Some(receiver),
        worker: Some(worker),
        stop,
        control,
    };
    match reader.receive()? {
        Some(Message::Schema(schema)) => {
            reader.schema = schema;
            Ok(reader)
        }
        Some(Message::Failed(error)) => Err(error),
        _ => Err(SqlSourceError::InconsistentRowShape),
    }
}

impl SqlBatchReader {
    fn receive(&mut self) -> Result<Option<Message>, SqlSourceError> {
        loop {
            self.control.check()?;
            let Some(receiver) = &self.receiver else {
                return Ok(None);
            };
            match receiver.recv_timeout(Duration::from_millis(20)) {
                Ok(message) => return Ok(Some(message)),
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
    fn close(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.receiver.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Iterator for SqlBatchReader {
    type Item = Result<RecordBatch, ArrowError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.as_ref()?;
        let error = match self.receive() {
            Ok(Some(Message::Batch(batch))) => return Some(Ok(batch)),
            Ok(Some(Message::Finished)) => {
                self.close();
                return None;
            }
            Ok(None) => SqlSourceError::RuntimePanicked,
            Ok(Some(Message::Failed(error))) | Err(error) => error,
            Ok(Some(Message::Schema(_))) => SqlSourceError::InconsistentRowShape,
        };
        self.close();
        Some(Err(ArrowError::ExternalError(Box::new(error))))
    }
}
impl RecordBatchReader for SqlBatchReader {
    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }
}
impl Drop for SqlBatchReader {
    fn drop(&mut self) {
        self.close();
    }
}
