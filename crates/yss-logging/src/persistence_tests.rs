use std::{collections::BTreeMap, sync::mpsc, time::Duration};

use crate::collector::LogLayer;
use sqlx::{ConnectOptions, Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};
use tracing_subscriber::prelude::*;

use crate::{LogDomain, LogLevel, LogRecordDto, LogRuntime};

#[test]
fn logs_commit_before_delivery_and_survive_restart_with_sqlx_readable_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(crate::LOG_DATABASE_NAME);
    let mut store = crate::store::LogStore::open(&path).unwrap();
    let stream_id = store.snapshot(1).unwrap().stream_id;
    store
        .append(&[LogRecordDto {
            stream_id,
            sequence: 1,
            timestamp: "2026-10-10T12:00:00.000".into(),
            level: LogLevel::Info,
            domain: LogDomain::Ui,
            target: "workbench.test".into(),
            event: Some("dropRejected".into()),
            message: "Drop was rejected".into(),
            source: None,
            fields: BTreeMap::from([("nodeCount".into(), serde_json::json!(3))]),
        }])
        .unwrap();
    drop(store);
    let logs = LogRuntime::open(path.clone()).unwrap();
    let (sender, receiver) = mpsc::channel();
    let snapshot = logs
        .subscribe_batches(move |batch| sender.send(batch).is_ok())
        .unwrap();
    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(snapshot.entries[0].domain, LogDomain::Ui);
    let subscriber = tracing_subscriber::registry().with(LogLayer::new(logs.rust_log_sink(None)));
    let dispatch = tracing::Dispatch::new(subscriber);
    tracing::dispatcher::with_default(&dispatch, || {
        tracing::warn!(
            target: "arbitrary_crate::worker",
            log_domain = "data",
            log_event = "missingValues",
            missing_count = 3,
            "Dataset contains missing values"
        );
    });
    let mut delivered = Vec::new();
    while delivered.is_empty() {
        let batch = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(batch.failure.is_none());
        delivered.extend(batch.entries);
    }

    let reader = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    reader.block_on(async {
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .read_only(true)
            .disable_statement_logging();
        let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
        let rows = sqlx::query(
            "SELECT sequence, domain, event, message, fields FROM logs ORDER BY sequence",
        )
        .fetch_all(&mut connection)
        .await
        .unwrap();
        assert_eq!(
            rows.len(),
            snapshot.entries.len() + delivered.len(),
            "snapshot and channel only announce committed rows"
        );
        assert_eq!(rows[1].get::<String, _>("domain"), "data");
        assert_eq!(rows[1].get::<String, _>("event"), "missingValues");
        assert_eq!(rows[0].get::<String, _>("domain"), "ui");
        assert_eq!(rows[0].get::<String, _>("event"), "dropRejected");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rows[1].get::<String, _>("fields")).unwrap()
                ["missing_count"],
            3
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rows[0].get::<String, _>("fields")).unwrap()
                ["nodeCount"],
            3
        );
        connection.close().await.unwrap();
    });

    // Shutdown must drain accepted entries even while a LogRuntime clone is retained by the desktop host.
    tracing::dispatcher::with_default(&dispatch, || {
        tracing::info!("Last before exit");
    });
    logs.shutdown();
    let reopened = LogRuntime::open(path.clone()).unwrap();
    let restored = reopened.subscribe_batches(|_| true).unwrap();
    assert_eq!(restored.stream_id, snapshot.stream_id);
    assert_eq!(restored.latest_sequence, 3);
    assert_eq!(restored.entries[1].event.as_deref(), Some("missingValues"));
    assert_eq!(restored.entries[1].fields["missing_count"], 3);
    assert_eq!(restored.entries[0].event.as_deref(), Some("dropRejected"));
    assert_eq!(restored.entries[2].message, "Last before exit");
    assert_eq!(restored.entries.len(), 3);
    reopened.shutdown();

    let mut storage = crate::store::LogStore::open(path).unwrap();
    let limited = storage.snapshot(2).unwrap();
    assert!(limited.truncated);
    assert_eq!(limited.latest_sequence, 3);
    assert_eq!(
        limited
            .entries
            .iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert!(!storage.snapshot(3).unwrap().truncated);
}

#[test]
fn failed_sqlite_write_sends_a_terminal_failure_without_publishing_uncommitted_logs() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(crate::LOG_DATABASE_NAME);
    let logs = LogRuntime::open(path.clone()).unwrap();
    let (sender, receiver) = mpsc::channel();
    logs.subscribe_batches(move |batch| sender.send(batch).is_ok())
        .unwrap();
    let reader = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    reader.block_on(async {
        let mut connection = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path).disable_statement_logging()).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER reject_log BEFORE INSERT ON logs BEGIN SELECT RAISE(FAIL, 'test failure'); END;")
            .execute(&mut connection).await.unwrap();
        connection.close().await.unwrap();
    });
    let subscriber = tracing_subscriber::registry().with(LogLayer::new(logs.rust_log_sink(None)));
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!("Cannot commit");
    });
    let failure = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(
        failure.failure,
        Some(crate::LogStreamFailure::StorageUnavailable)
    );
    assert!(failure.entries.is_empty());
    assert!(matches!(
        receiver.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    assert!(logs.subscribe_batches(|_| true).is_err());
    let (console_sender, console_receiver) = mpsc::channel();
    let (console, _console_guard) = crate::collector::spawn_output(
        "storage-failure-test",
        Box::new(move |record| console_sender.send(record.clone()).is_ok()),
    )
    .unwrap();
    let subscriber =
        tracing_subscriber::registry().with(LogLayer::new(logs.rust_log_sink(Some(console))));
    tracing::subscriber::with_default(subscriber, || {
        tracing::warn!(password = "hidden", "Authorization: Bearer secret");
    });
    logs.shutdown();
    let record = console_receiver
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    assert!(!record.message.contains("secret"));
    assert_eq!(record.fields["password"], "[REDACTED]");
    let mut storage = crate::store::LogStore::open(path).unwrap();
    assert!(storage.snapshot(1).unwrap().entries.is_empty());
}
