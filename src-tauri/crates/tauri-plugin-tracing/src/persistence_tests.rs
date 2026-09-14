use std::{collections::BTreeMap, sync::mpsc, time::Duration};

use crate::collector::LogLayer;
use sqlx::{ConnectOptions, Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};
use tracing_subscriber::prelude::*;

use crate::{FrontendLogEntryDto, LogDomain, LogLevel, LogOrigin, LogQuery, LogRuntime};

fn frontend(message: &str) -> FrontendLogEntryDto {
    FrontendLogEntryDto {
        level: LogLevel::Info,
        domain: LogDomain::Ui,
        target: "frontend.test".into(),
        event: None,
        message: message.into(),
        source: None,
        fields: BTreeMap::new(),
    }
}

#[test]
fn logs_commit_before_delivery_and_survive_restart_with_sqlx_readable_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(crate::LOG_DATABASE_NAME);
    let logs = LogRuntime::open(path.clone()).unwrap();
    let (sender, receiver) = mpsc::channel();
    let snapshot = logs
        .subscribe_batches(move |batch| sender.send(batch).is_ok())
        .unwrap();
    assert!(snapshot.entries.is_empty());
    let subscriber = tracing_subscriber::registry().with(LogLayer::new(logs.rust_log_sink(None)));
    let dispatch = tracing::Dispatch::new(subscriber);
    tracing::dispatcher::with_default(&dispatch, || {
        tracing::info!(target: "arbitrary_crate::worker", count = 1, "Rust worker ready");
    });
    logs.submit_frontend(vec![frontend("Frontend ready")])
        .unwrap();
    let mut delivered = Vec::new();
    while delivered.len() < 2 {
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
        let rows =
            sqlx::query("SELECT sequence, origin, message, fields FROM logs ORDER BY sequence")
                .fetch_all(&mut connection)
                .await
                .unwrap();
        assert_eq!(
            rows.len(),
            delivered.len(),
            "channel only announces committed rows"
        );
        assert_eq!(rows[0].get::<String, _>("origin"), "rust");
        assert_eq!(rows[1].get::<String, _>("origin"), "frontend");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&rows[0].get::<String, _>("fields")).unwrap()
                ["count"],
            1
        );
        connection.close().await.unwrap();
    });

    // Shutdown must drain accepted entries even while a LogRuntime clone is retained by Tauri.
    tracing::dispatcher::with_default(&dispatch, || {
        tracing::info!("Last before exit");
    });
    logs.shutdown();
    let reopened = LogRuntime::open(path).unwrap();
    let restored = reopened.subscribe_batches(|_| true).unwrap();
    assert_eq!(restored.stream_id, snapshot.stream_id);
    assert_eq!(restored.latest_sequence, 3);
    assert_eq!(restored.entries[2].message, "Last before exit");
    let page = reopened
        .query(LogQuery {
            limit: 2,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        page.entries
            .iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    let previous = reopened
        .query(LogQuery {
            before_sequence: page.next_before_sequence,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(previous.entries[0].sequence, 1);
    assert!(previous.next_before_sequence.is_none());
    let rust_only = reopened
        .query(LogQuery {
            origin: Some(LogOrigin::Rust),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(rust_only.entries.len(), 2);
    let statistics = reopened.statistics().unwrap();
    assert_eq!(statistics.total, 3);
    assert_eq!(statistics.by_origin["frontend"], 1);
    reopened.shutdown();
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
    logs.submit_frontend(vec![frontend("Cannot commit")])
        .unwrap();
    let failure = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(failure.failure.as_deref(), Some("storage_unavailable"));
    assert!(failure.entries.is_empty());
    assert!(logs.query(LogQuery::default()).is_err());
    assert!(logs.subscribe_batches(|_| true).is_err());
    assert!(
        logs.submit_frontend(vec![frontend("Rejected after failure")])
            .is_err()
    );
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
}
