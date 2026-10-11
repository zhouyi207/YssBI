use std::{collections::BTreeMap, path::Path, sync::mpsc, time::Duration};

use chrono::NaiveDateTime;
use sqlx::{ConnectOptions, Connection, SqliteConnection, sqlite::SqliteConnectOptions};

use crate::{
    FrontendLogEntryDto, LogDomain, LogLevel, LogOrigin, LogQuery, LogRecordDto,
    LogRetentionPolicy, LogRuntime, LogStoreError, LogStreamFailure, store::LogStore,
};

fn now() -> NaiveDateTime {
    NaiveDateTime::parse_from_str("2026-10-10T12:00:00.000", "%Y-%m-%dT%H:%M:%S%.f").unwrap()
}

fn record(stream: &str, sequence: u64, timestamp: &str) -> LogRecordDto {
    LogRecordDto {
        stream_id: stream.into(),
        sequence,
        timestamp: timestamp.into(),
        level: LogLevel::Info,
        origin: LogOrigin::Rust,
        domain: LogDomain::System,
        target: "retention.test".into(),
        event: None,
        message: "retained record".into(),
        source: None,
        fields: BTreeMap::new(),
    }
}

fn execute(path: &Path, sql: &'static str) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .disable_statement_logging(),
        )
        .await
        .unwrap();
        sqlx::query(sql).execute(&mut connection).await.unwrap();
        connection.close().await.unwrap();
    });
}

fn sqlite_pages(path: &Path) -> (u64, u64, u64) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .disable_statement_logging(),
        )
        .await
        .unwrap();
        let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        let free: i64 = sqlx::query_scalar("PRAGMA freelist_count")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        let size: i64 = sqlx::query_scalar("PRAGMA page_size")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
        (pages as u64, free as u64, size as u64)
    })
}

#[test]
fn policy_bounds_and_default_disabled_are_explicit() {
    assert!(!LogRetentionPolicy::default().is_enabled());
    assert!(LogRetentionPolicy::new(Some(1), Some(1)).is_ok());
    assert!(LogRetentionPolicy::new(Some(3650), Some(10240)).is_ok());
    for (days, size) in [
        (Some(0), None),
        (Some(3651), None),
        (None, Some(0)),
        (None, Some(10241)),
    ] {
        assert!(matches!(
            LogRetentionPolicy::new(days, size),
            Err(LogStoreError::InvalidRetention)
        ));
    }
}

#[test]
fn age_cutoff_is_strict_and_handles_non_monotonic_timestamps_and_cursors() {
    let directory = tempfile::tempdir().unwrap();
    let mut store = LogStore::open(directory.path().join("logs.sqlite")).unwrap();
    let stream = store.snapshot(10).unwrap().stream_id;
    store
        .append(&[
            record(&stream, 1, "2026-10-09T11:59:59.999"),
            record(&stream, 2, "2026-10-09T12:00:00.000"),
            record(&stream, 3, "2026-10-10T12:00:00.000"),
            record(&stream, 4, "2026-10-08T12:00:00.000"),
        ])
        .unwrap();
    assert!(store.maintain(now()).unwrap().evicted.is_empty());
    assert_eq!(
        store.statistics().unwrap().total,
        4,
        "disabled means full history"
    );
    let cursor = store
        .query(LogQuery {
            limit: 2,
            ..Default::default()
        })
        .unwrap()
        .next_before_sequence;
    assert_eq!(cursor, Some(3));
    store
        .set_retention(LogRetentionPolicy::new(Some(1), None).unwrap())
        .unwrap();
    assert_eq!(store.maintain(now()).unwrap().evicted, vec![1, 4]);
    let snapshot = store.snapshot(10).unwrap();
    assert_eq!(
        snapshot
            .entries
            .iter()
            .map(|entry| entry.sequence)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(snapshot.latest_sequence, 4);
    assert!(snapshot.truncated);
    let previous = store
        .query(LogQuery {
            before_sequence: cursor,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        previous
            .entries
            .iter()
            .map(|entry| entry.sequence)
            .collect::<Vec<_>>(),
        vec![2]
    );
    assert!(previous.next_before_sequence.is_none());
    store.set_retention(LogRetentionPolicy::default()).unwrap();
    assert!(
        store
            .maintain(now() + chrono::Duration::days(100))
            .unwrap()
            .evicted
            .is_empty()
    );
}

#[test]
fn emptying_history_preserves_sequence_and_stream_after_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("logs.sqlite");
    let mut store = LogStore::open(&path).unwrap();
    let stream = store.snapshot(10).unwrap().stream_id;
    store
        .append(&[record(&stream, 1, "2000-01-01T00:00:00.000")])
        .unwrap();
    store
        .set_retention(LogRetentionPolicy::new(Some(1), None).unwrap())
        .unwrap();
    assert_eq!(store.maintain(now()).unwrap().evicted, vec![1]);
    assert_eq!(store.statistics().unwrap().total, 0);
    assert_eq!(store.statistics().unwrap().latest_sequence, 1);
    drop(store);
    let mut reopened = LogStore::open(&path).unwrap();
    let snapshot = reopened.snapshot(10).unwrap();
    assert_eq!(snapshot.stream_id, stream);
    assert_eq!(snapshot.latest_sequence, 1);
    assert!(snapshot.entries.is_empty());
    assert!(snapshot.truncated);
    assert!(matches!(
        reopened.append(&[record(&stream, 1, "2026-10-10T12:00:00.000")]),
        Err(LogStoreError::InvalidSequence)
    ));
    reopened
        .append(&[record(&stream, 2, "2026-10-10T12:00:00.000")])
        .unwrap();
    assert_eq!(reopened.snapshot(10).unwrap().latest_sequence, 2);
}

#[test]
fn age_maintenance_has_a_bounded_deletion_budget_and_converges() {
    let directory = tempfile::tempdir().unwrap();
    let mut store = LogStore::open(directory.path().join("logs.sqlite")).unwrap();
    let stream = store.snapshot(10).unwrap().stream_id;
    let entries = (1..=513)
        .map(|sequence| record(&stream, sequence, "2000-01-01T00:00:00.000"))
        .collect::<Vec<_>>();
    store.append(&entries).unwrap();
    store
        .set_retention(LogRetentionPolicy::new(Some(1), None).unwrap())
        .unwrap();
    let first = store.maintain(now()).unwrap();
    assert_eq!(first.evicted.len(), 512);
    assert!(first.pending);
    assert_eq!(store.statistics().unwrap().total, 1);
    assert_eq!(store.maintain(now()).unwrap().evicted, vec![513]);
    assert_eq!(store.statistics().unwrap().latest_sequence, 513);
}

#[test]
fn storage_target_evicts_oldest_and_reclaims_database_and_wal_space() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("logs.sqlite");
    let mut store = LogStore::open(&path).unwrap();
    store
        .set_retention(LogRetentionPolicy::new(None, Some(1)).unwrap())
        .unwrap();
    let stream = store.snapshot(10).unwrap().stream_id;
    let entries = (1..=600)
        .map(|sequence| {
            let mut entry = record(&stream, sequence, "2026-10-10T12:00:00.000");
            entry.message = "x".repeat(8192);
            entry
        })
        .collect::<Vec<_>>();
    store.append(&entries).unwrap();
    let mut completed = false;
    for _ in 0..128 {
        let outcome = store.maintain(now()).unwrap();
        assert!(outcome.evicted.len() <= 512);
        if !outcome.pending {
            completed = true;
            break;
        }
    }
    assert!(
        completed,
        "bounded maintenance must converge without pinned readers"
    );
    let snapshot = store.snapshot(1000).unwrap();
    assert!(!snapshot.entries.is_empty());
    assert!(snapshot.entries.len() < 600);
    assert_eq!(snapshot.entries.last().unwrap().sequence, 600);
    assert!(
        snapshot
            .entries
            .windows(2)
            .all(|pair| pair[0].sequence + 1 == pair[1].sequence)
    );
    let (pages, free, size) = sqlite_pages(&path);
    assert!((pages - free) * size <= 1024 * 1024);
    assert_eq!(free, 0, "incremental vacuum releases free database pages");
    assert!(std::fs::metadata(&path).unwrap().len() <= 1024 * 1024);
    assert_eq!(
        std::fs::metadata(path.with_extension("sqlite-wal"))
            .unwrap()
            .len(),
        0
    );
    // Below target is a no-op, including at the next maintenance boundary.
    assert!(store.maintain(now()).unwrap().evicted.is_empty());
}

#[test]
fn runtime_policy_changes_keep_live_subscribers_and_high_water_after_eviction() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("logs.sqlite");
    let mut store = LogStore::open(&path).unwrap();
    let stream = store.snapshot(10).unwrap().stream_id;
    store
        .append(&[record(&stream, 1, "2000-01-01T00:00:00.000")])
        .unwrap();
    drop(store);
    let logs = LogRuntime::open(path.clone()).unwrap();
    let (sender, receiver) = mpsc::channel();
    let subscription = logs
        .subscribe_batches(move |batch| sender.send(batch).is_ok())
        .unwrap();
    assert_eq!(subscription.entries.len(), 1);
    logs.set_retention(LogRetentionPolicy::new(Some(1), None).unwrap())
        .unwrap();
    let eviction = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(eviction.evicted_sequences, vec![1]);
    assert!(eviction.entries.is_empty());
    assert!(eviction.failure.is_none());
    assert_eq!(eviction.stream_id, stream);
    assert_eq!(logs.statistics().unwrap().latest_sequence, 1);
    assert!(logs.query(LogQuery::default()).unwrap().entries.is_empty());
    let snapshot = logs.subscribe_batches(|_| true).unwrap();
    assert!(snapshot.entries.is_empty());
    assert_eq!(snapshot.latest_sequence, 1);
    logs.submit_frontend(vec![FrontendLogEntryDto {
        level: LogLevel::Info,
        domain: LogDomain::Ui,
        target: "retention.test".into(),
        event: None,
        message: "next record".into(),
        source: None,
        fields: BTreeMap::new(),
    }])
    .unwrap();
    let next = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(next.failure.is_none());
    assert_eq!(next.entries[0].sequence, 2);
    assert_eq!(next.stream_id, stream);
    logs.unsubscribe(subscription.subscription_id).unwrap();
    logs.shutdown();
    let reopened =
        LogRuntime::open_with_retention(path, LogRetentionPolicy::new(Some(1), None).unwrap())
            .unwrap();
    assert_eq!(reopened.statistics().unwrap().latest_sequence, 2);
    reopened.shutdown();
}

#[test]
fn maintenance_failure_is_returned_and_delivered_without_false_eviction() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("logs.sqlite");
    let mut store = LogStore::open(&path).unwrap();
    let stream = store.snapshot(10).unwrap().stream_id;
    store
        .append(&[record(&stream, 1, "2000-01-01T00:00:00.000")])
        .unwrap();
    drop(store);
    execute(
        &path,
        "CREATE TRIGGER reject_eviction BEFORE DELETE ON logs BEGIN SELECT RAISE(FAIL, 'retention failure'); END;",
    );
    let logs = LogRuntime::open(path.clone()).unwrap();
    let (sender, receiver) = mpsc::channel();
    logs.subscribe_batches(move |batch| sender.send(batch).is_ok())
        .unwrap();
    assert!(
        logs.set_retention(LogRetentionPolicy::new(Some(1), None).unwrap())
            .is_err()
    );
    let failure = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(failure.failure, Some(LogStreamFailure::RetentionFailed));
    assert!(failure.entries.is_empty());
    assert!(failure.evicted_sequences.is_empty());
    assert!(logs.query(LogQuery::default()).is_err());
    logs.shutdown();
    assert!(
        LogRuntime::open_with_retention(
            path.clone(),
            LogRetentionPolicy::new(Some(1), None).unwrap()
        )
        .is_err()
    );
    let mut store = LogStore::open(path).unwrap();
    assert_eq!(
        store.statistics().unwrap().total,
        1,
        "failed deletion rolls back"
    );
}

#[test]
fn startup_retention_runs_before_snapshot_and_resumes_after_empty_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("logs.sqlite");
    let mut store = LogStore::open(&path).unwrap();
    let stream = store.snapshot(10).unwrap().stream_id;
    store
        .append(&[record(&stream, 1, "2000-01-01T00:00:00.000")])
        .unwrap();
    drop(store);
    let logs =
        LogRuntime::open_with_retention(path, LogRetentionPolicy::new(Some(1), None).unwrap())
            .unwrap();
    let snapshot = logs.subscribe_batches(|_| true).unwrap();
    assert_eq!(snapshot.stream_id, stream);
    assert_eq!(snapshot.latest_sequence, 1);
    assert!(snapshot.entries.is_empty());
    logs.submit_frontend(vec![FrontendLogEntryDto {
        level: LogLevel::Info,
        domain: LogDomain::Ui,
        target: "retention.test".into(),
        event: None,
        message: "after startup".into(),
        source: None,
        fields: BTreeMap::new(),
    }])
    .unwrap();
    assert_eq!(logs.statistics().unwrap().latest_sequence, 2);
    logs.shutdown();
}

#[test]
fn pinned_reader_defers_reclamation_without_losing_its_snapshot_or_failing_writes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("logs.sqlite");
    let mut store = LogStore::open(&path).unwrap();
    store
        .set_retention(LogRetentionPolicy::new(Some(1), None).unwrap())
        .unwrap();
    let stream = store.snapshot(10).unwrap().stream_id;
    store
        .append(&[record(&stream, 1, "2000-01-01T00:00:00.000")])
        .unwrap();
    let reader = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut connection = reader.block_on(async {
        let mut connection = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&path)
                .read_only(true)
                .disable_statement_logging(),
        )
        .await
        .unwrap();
        sqlx::query("BEGIN").execute(&mut connection).await.unwrap();
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logs")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(total, 1);
        connection
    });
    let outcome = store.maintain(now()).unwrap();
    assert_eq!(outcome.evicted, vec![1]);
    assert!(
        outcome.pending,
        "WAL reclamation waits for the reader, not the dispatcher"
    );
    assert_eq!(store.statistics().unwrap().total, 0);
    store
        .append(&[record(&stream, 2, "2026-10-10T12:00:00.000")])
        .unwrap();
    reader.block_on(async {
        let sequence: i64 = sqlx::query_scalar("SELECT sequence FROM logs")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(
            sequence, 1,
            "existing read transactions retain SQLite snapshot isolation"
        );
        sqlx::query("ROLLBACK")
            .execute(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
    });
    assert!(!store.maintain(now()).unwrap().pending);
    assert_eq!(store.statistics().unwrap().latest_sequence, 2);
}
