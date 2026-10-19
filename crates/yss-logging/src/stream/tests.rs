use std::collections::BTreeMap;
use std::sync::mpsc;
use std::time::Duration;

use crate::collector::CapturedLog;
use chrono::NaiveDateTime;
use serde_json::json;

use super::dispatcher::{LIVE_BATCH_INTERVAL, LIVE_BATCH_MAX_RECORDS, LogHub, RECENT_LOG_CAPACITY};
use super::dto::{LogDomain, LogLevel, LogRecordDto, LogStreamFailure};

fn captured(message: impl Into<String>) -> CapturedLog {
    CapturedLog {
        timestamp: chrono::Local::now().naive_local(),
        level: LogLevel::Info,
        target: "yssbi::test".into(),
        message: message.into(),
        fields: BTreeMap::new(),
    }
}

#[test]
fn log_record_serializes_strict_camel_case_contract() {
    let record = LogRecordDto {
        stream_id: "stream-1".into(),
        sequence: 7,
        timestamp: super::local_timestamp_now(),
        level: LogLevel::Warn,
        domain: LogDomain::Execution,
        target: "yssbi::runtime".into(),
        event: Some("runFailed".into()),
        message: "run failed".into(),
        source: None,
        fields: BTreeMap::from([("runId".into(), json!(42))]),
    };

    let value = serde_json::to_value(&record).unwrap();
    assert_eq!(value["streamId"], "stream-1");
    assert_eq!(value["sequence"], 7);
    assert_eq!(value["level"], "warn");
    assert_eq!(value["domain"], "execution");
    assert_eq!(value["event"], "runFailed");
    assert_eq!(value["fields"]["runId"], 42);
    assert!(value.get("stream_id").is_none());
    assert!(value.get("source").is_none());
    NaiveDateTime::parse_from_str(value["timestamp"].as_str().unwrap(), "%Y-%m-%dT%H:%M:%S%.f")
        .unwrap();
}

#[test]
fn log_domains_serialize_to_the_six_lowercase_values() {
    let domains = [
        (LogDomain::Application, "application"),
        (LogDomain::Execution, "execution"),
        (LogDomain::System, "system"),
        (LogDomain::Graph, "graph"),
        (LogDomain::Data, "data"),
        (LogDomain::Ui, "ui"),
    ];
    for (domain, expected) in domains {
        assert_eq!(serde_json::to_value(domain).unwrap(), expected);
    }
}

#[test]
fn recent_ring_keeps_5000_records_with_rust_owned_monotonic_sequences() {
    let (hub, _guard) = LogHub::start_for_test(RECENT_LOG_CAPACITY + 2, 8);
    for index in 0..=RECENT_LOG_CAPACITY {
        hub.publish_rust(captured(format!("record-{index}")), None)
            .unwrap();
    }

    let subscription = hub.subscribe(|_| true).unwrap();
    assert_eq!(subscription.entries.len(), RECENT_LOG_CAPACITY);
    assert_eq!(subscription.entries.first().unwrap().sequence, 2);
    assert_eq!(
        subscription.entries.last().unwrap().sequence,
        (RECENT_LOG_CAPACITY + 1) as u64
    );
    assert_eq!(
        subscription.latest_sequence,
        (RECENT_LOG_CAPACITY + 1) as u64
    );
    assert!(subscription.truncated);
    assert!(
        subscription
            .entries
            .iter()
            .all(|record| record.stream_id == subscription.stream_id)
    );
    hub.unsubscribe(subscription.subscription_id).unwrap();
}

#[test]
fn subscription_snapshot_and_live_batches_have_one_ordered_boundary() {
    let (hub, _guard) = LogHub::start();
    hub.publish_rust(captured("snapshot"), None).unwrap();
    let (batch_sender, batch_receiver) = mpsc::channel();
    let subscription = hub
        .subscribe(move |batch| batch_sender.send(batch).is_ok())
        .unwrap();

    hub.publish_rust(captured("live-1"), None).unwrap();
    hub.publish_rust(captured("live-2"), None).unwrap();
    let live = batch_receiver.recv_timeout(Duration::from_secs(1)).unwrap();

    assert_eq!(
        subscription
            .entries
            .iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(
        live.entries
            .iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(live.stream_id, subscription.stream_id);
    assert!(subscription.entries.iter().all(|snapshot| {
        live.entries
            .iter()
            .all(|record| record.sequence != snapshot.sequence)
    }));

    hub.unsubscribe(subscription.subscription_id).unwrap();
    hub.publish_rust(captured("after-unsubscribe"), None)
        .unwrap();
    assert!(
        batch_receiver
            .recv_timeout(Duration::from_millis(50))
            .is_err()
    );
}

#[test]
fn overflowing_subscriber_stops_without_delivering_queued_batches() {
    let (hub, _guard) = LogHub::start_for_test(16, 1);
    let (healthy_sender, healthy_receiver) = mpsc::channel();
    let healthy = hub
        .subscribe(move |batch| healthy_sender.send(batch).is_ok())
        .unwrap();
    let (started_sender, started_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let (slow_sender, slow_receiver) = mpsc::channel();
    let _slow = hub
        .subscribe(move |batch| {
            if batch
                .entries
                .first()
                .is_some_and(|entry| entry.message == "in-flight")
            {
                let _ = started_sender.send(());
                if release_receiver
                    .recv_timeout(Duration::from_secs(2))
                    .is_err()
                {
                    return false;
                }
            }
            slow_sender.send(batch).is_ok()
        })
        .unwrap();

    hub.publish_rust(captured("in-flight"), None).unwrap();
    started_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    healthy_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();

    for (sequence, message) in [(2, "queued"), (3, "overflow")] {
        hub.publish_rust(captured(message), None).unwrap();
        let batch = healthy_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        assert_eq!(batch.entries[0].sequence, sequence);
    }
    // The acknowledgement comes after the dispatcher has handled the overflow.
    hub.unsubscribe(healthy.subscription_id).unwrap();
    release_sender.send(()).unwrap();

    let first = slow_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(first.entries[0].message, "in-flight");
    let terminal = slow_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(terminal.entries.is_empty());
    assert_eq!(terminal.failure, Some(LogStreamFailure::SubscriberLagged));
    assert!(matches!(
        slow_receiver.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    let recovered = hub.subscribe(|_| true).unwrap();
    assert_eq!(recovered.stream_id, first.stream_id);
    assert_eq!(recovered.latest_sequence, 3);
    assert_eq!(
        recovered
            .entries
            .iter()
            .map(|entry| entry.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}

#[test]
fn live_batches_preserve_order_and_flush_by_size_or_time() {
    let (hub, _guard) = LogHub::start();
    let (batch_sender, batch_receiver) = mpsc::channel();
    let subscription = hub
        .subscribe(move |batch| batch_sender.send(batch).is_ok())
        .unwrap();

    let started = std::time::Instant::now();
    for index in 0..LIVE_BATCH_MAX_RECORDS + 1 {
        hub.publish_rust(captured(format!("batch-{index}")), None)
            .unwrap();
    }

    let size_batch = batch_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(size_batch.entries.len(), LIVE_BATCH_MAX_RECORDS);
    assert_eq!(size_batch.entries.first().unwrap().sequence, 1);
    assert_eq!(
        size_batch.entries.last().unwrap().sequence,
        LIVE_BATCH_MAX_RECORDS as u64
    );

    let timed_batch = batch_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(timed_batch.entries.len(), 1);
    assert_eq!(
        timed_batch.entries[0].sequence,
        (LIVE_BATCH_MAX_RECORDS + 1) as u64
    );
    assert!(started.elapsed() >= LIVE_BATCH_INTERVAL);
    assert_eq!(size_batch.stream_id, subscription.stream_id);
    assert_eq!(timed_batch.stream_id, subscription.stream_id);

    hub.unsubscribe(subscription.subscription_id).unwrap();
}

#[test]
fn bounded_ingress_reports_exact_drop_count_on_recovery() {
    let (hub, _guard, release_dispatcher) = LogHub::start_paused_for_test(2);

    hub.publish_rust(captured("first"), None).unwrap();
    hub.publish_rust(captured("queued"), None).unwrap();
    let overflow_started = std::time::Instant::now();
    hub.publish_rust(captured("dropped-1"), None).unwrap();
    hub.publish_rust(captured("dropped-2"), None).unwrap();
    assert!(overflow_started.elapsed() < Duration::from_millis(100));
    release_dispatcher.send(()).unwrap();

    let recovery_started = std::time::Instant::now();
    let snapshot = loop {
        if let Ok(snapshot) = hub.subscribe(|_| true) {
            // A full ingress queue can defer its drop marker until after the first snapshot.
            if snapshot.latest_sequence >= 3 {
                break snapshot;
            }
            hub.unsubscribe(snapshot.subscription_id).unwrap();
        }
        assert!(recovery_started.elapsed() < Duration::from_secs(1));
        std::thread::sleep(Duration::from_millis(5));
    };
    let first = &snapshot.entries[0];
    let queued = &snapshot.entries[1];
    let dropped = &snapshot.entries[2];
    assert_eq!(first.sequence, 1);
    assert_eq!(queued.sequence, 2);
    assert_eq!(queued.message, "queued");
    assert_eq!(dropped.sequence, 3);
    assert_eq!(dropped.event.as_deref(), Some("logs.records_dropped"));
    assert_eq!(dropped.fields["droppedCount"], 2);

    assert_eq!(snapshot.latest_sequence, 3);
    assert_eq!(
        snapshot
            .entries
            .iter()
            .filter(|record| record.event.as_deref() == Some("logs.records_dropped"))
            .count(),
        1
    );
    hub.unsubscribe(snapshot.subscription_id).unwrap();
}

#[test]
fn shutdown_does_not_wait_for_full_ingress() {
    let (hub, guard, _release_dispatcher) = LogHub::start_paused_for_test(1);
    hub.publish_rust(captured("queued"), None).unwrap();
    hub.publish_rust(captured("dropped"), None).unwrap();
    let started = std::time::Instant::now();
    drop(guard);
    assert!(started.elapsed() < Duration::from_millis(500));
}
