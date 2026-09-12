use std::collections::BTreeMap;
use std::sync::mpsc;
use std::time::Duration;

use chrono::NaiveDateTime;
use serde_json::json;

use super::dispatcher::{
    DiagnosticsHub, LIVE_BATCH_INTERVAL, LIVE_BATCH_MAX_RECORDS, PendingDiagnostic,
    RECENT_DIAGNOSTIC_CAPACITY,
};
use super::dto::{
    DiagnosticBatchDto, DiagnosticDomain, DiagnosticLevel, DiagnosticOrigin, DiagnosticRecordDto,
    DiagnosticSubscriptionDto, FrontendDiagnosticEntryDto,
};
use super::validation::{MAX_FRONTEND_DIAGNOSTIC_BATCH, validate_frontend_batch};

fn pending(message: impl Into<String>) -> PendingDiagnostic {
    PendingDiagnostic {
        timestamp: super::local_timestamp_now(),
        level: DiagnosticLevel::Info,
        origin: DiagnosticOrigin::Rust,
        domain: DiagnosticDomain::Application,
        target: "yssbi::test".into(),
        event: None,
        message: message.into(),
        source: None,
        fields: BTreeMap::new(),
    }
}

fn frontend_entry() -> FrontendDiagnosticEntryDto {
    FrontendDiagnosticEntryDto {
        level: DiagnosticLevel::Warn,
        domain: DiagnosticDomain::Ui,
        target: "editor.canvas".into(),
        event: Some("dropRejected".into()),
        message: "Drop was rejected".into(),
        source: Some("main-window".into()),
        fields: BTreeMap::from([("nodeCount".into(), json!(3))]),
    }
}

#[test]
fn diagnostic_record_serializes_strict_camel_case_contract() {
    let record = DiagnosticRecordDto {
        stream_id: "stream-1".into(),
        sequence: 7,
        timestamp: super::local_timestamp_now(),
        level: DiagnosticLevel::Warn,
        origin: DiagnosticOrigin::Rust,
        domain: DiagnosticDomain::Execution,
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
    assert_eq!(value["origin"], "rust");
    assert_eq!(value["domain"], "execution");
    assert_eq!(value["event"], "runFailed");
    assert_eq!(value["fields"]["runId"], 42);
    assert!(value.get("stream_id").is_none());
    assert!(value.get("source").is_none());
    NaiveDateTime::parse_from_str(value["timestamp"].as_str().unwrap(), "%Y-%m-%dT%H:%M:%S%.f")
        .unwrap();
}

#[test]
fn frontend_batches_are_bounded_and_validate_text_and_fields() {
    assert!(validate_frontend_batch(vec![frontend_entry()]).is_ok());
    assert!(validate_frontend_batch(Vec::new()).is_err());
    assert!(
        validate_frontend_batch(vec![frontend_entry(); MAX_FRONTEND_DIAGNOSTIC_BATCH + 1]).is_err()
    );

    let mut invalid_target = frontend_entry();
    invalid_target.target = "editor\0canvas".into();
    assert!(validate_frontend_batch(vec![invalid_target]).is_err());

    let mut too_deep = json!("leaf");
    for _ in 0..9 {
        too_deep = json!({ "nested": too_deep });
    }
    let mut invalid_fields = frontend_entry();
    invalid_fields.fields.insert("deep".into(), too_deep);
    assert!(validate_frontend_batch(vec![invalid_fields]).is_err());
}

#[test]
fn batch_and_subscription_serialize_exact_entries_contract() {
    let record = DiagnosticRecordDto {
        stream_id: "stream-1".into(),
        sequence: 1,
        timestamp: super::local_timestamp_now(),
        level: DiagnosticLevel::Info,
        origin: DiagnosticOrigin::Frontend,
        domain: DiagnosticDomain::Ui,
        target: "editor.canvas".into(),
        event: None,
        message: "ready".into(),
        source: None,
        fields: BTreeMap::new(),
    };
    let batch = serde_json::to_value(DiagnosticBatchDto {
        stream_id: "stream-1".into(),
        entries: vec![record.clone()],
    })
    .unwrap();
    assert_eq!(batch.as_object().unwrap().len(), 2);
    assert!(batch.get("entries").is_some());
    assert!(batch.get("records").is_none());

    let subscription = serde_json::to_value(DiagnosticSubscriptionDto {
        subscription_id: "subscription-1".into(),
        stream_id: "stream-1".into(),
        entries: vec![record],
        latest_sequence: 1,
        truncated: false,
    })
    .unwrap();
    assert_eq!(subscription.as_object().unwrap().len(), 5);
    assert!(subscription.get("entries").is_some());
    assert!(subscription.get("snapshot").is_none());
    assert_eq!(subscription["latestSequence"], 1);
}

#[test]
fn diagnostic_domains_serialize_to_the_six_lowercase_values() {
    let domains = [
        (DiagnosticDomain::Application, "application"),
        (DiagnosticDomain::Execution, "execution"),
        (DiagnosticDomain::System, "system"),
        (DiagnosticDomain::Graph, "graph"),
        (DiagnosticDomain::Data, "data"),
        (DiagnosticDomain::Ui, "ui"),
    ];
    for (domain, expected) in domains {
        assert_eq!(serde_json::to_value(domain).unwrap(), expected);
    }
}

#[test]
fn frontend_entry_deserialization_rejects_contract_drift() {
    let mut unknown_field = serde_json::to_value(frontend_entry()).unwrap();
    unknown_field
        .as_object_mut()
        .unwrap()
        .insert("unknown".into(), json!(true));
    assert!(serde_json::from_value::<FrontendDiagnosticEntryDto>(unknown_field).is_err());

    let mut missing_fields = serde_json::to_value(frontend_entry()).unwrap();
    missing_fields.as_object_mut().unwrap().remove("fields");
    assert!(serde_json::from_value::<FrontendDiagnosticEntryDto>(missing_fields).is_err());
    assert!(
        serde_json::from_value::<FrontendDiagnosticEntryDto>(json!({
            "level": "info",
            "domain": "notify",
            "target": "frontend.notify",
            "message": "legacy domain",
            "fields": {}
        }))
        .is_err()
    );
}

#[test]
fn recent_ring_keeps_5000_records_with_rust_owned_monotonic_sequences() {
    let (hub, _guard) = DiagnosticsHub::start_for_test(RECENT_DIAGNOSTIC_CAPACITY + 2, 8);
    hub.publish(
        (0..=RECENT_DIAGNOSTIC_CAPACITY)
            .map(|index| pending(format!("record-{index}")))
            .collect(),
    )
    .unwrap();

    let subscription = hub.subscribe(|_| true).unwrap();
    assert_eq!(subscription.entries.len(), RECENT_DIAGNOSTIC_CAPACITY);
    assert_eq!(subscription.entries.first().unwrap().sequence, 2);
    assert_eq!(
        subscription.entries.last().unwrap().sequence,
        (RECENT_DIAGNOSTIC_CAPACITY + 1) as u64
    );
    assert_eq!(
        subscription.latest_sequence,
        (RECENT_DIAGNOSTIC_CAPACITY + 1) as u64
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
    let (hub, _guard) = DiagnosticsHub::start();
    hub.publish(vec![pending("snapshot")]).unwrap();
    let (batch_sender, batch_receiver) = mpsc::channel();
    let subscription = hub
        .subscribe(move |batch| batch_sender.send(batch).is_ok())
        .unwrap();

    hub.publish(vec![pending("live-1"), pending("live-2")])
        .unwrap();
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
    hub.publish(vec![pending("after-unsubscribe")]).unwrap();
    assert!(
        batch_receiver
            .recv_timeout(Duration::from_millis(50))
            .is_err()
    );
}

#[test]
fn live_batches_preserve_order_and_flush_by_size_or_time() {
    let (hub, _guard) = DiagnosticsHub::start();
    let (batch_sender, batch_receiver) = mpsc::channel();
    let subscription = hub
        .subscribe(move |batch| batch_sender.send(batch).is_ok())
        .unwrap();

    let started = std::time::Instant::now();
    hub.publish(
        (0..LIVE_BATCH_MAX_RECORDS + 1)
            .map(|index| pending(format!("batch-{index}")))
            .collect(),
    )
    .unwrap();

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
    let (hub, _guard, release_dispatcher) = DiagnosticsHub::start_paused_for_test(2);

    hub.publish(vec![pending("first"), pending("queued")])
        .unwrap();
    let overflow_started = std::time::Instant::now();
    hub.publish(vec![pending("dropped-1"), pending("dropped-2")])
        .unwrap();
    assert!(overflow_started.elapsed() < Duration::from_millis(100));
    release_dispatcher.send(()).unwrap();

    let recovery_started = std::time::Instant::now();
    let snapshot = loop {
        if let Ok(snapshot) = hub.subscribe(|_| true) {
            break snapshot;
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
    assert_eq!(
        dropped.event.as_deref(),
        Some("diagnostics.records_dropped")
    );
    assert_eq!(dropped.fields["droppedCount"], 2);

    assert_eq!(snapshot.latest_sequence, 3);
    assert_eq!(
        snapshot
            .entries
            .iter()
            .filter(|record| record.event.as_deref() == Some("diagnostics.records_dropped"))
            .count(),
        1
    );
    hub.unsubscribe(snapshot.subscription_id).unwrap();
}

#[test]
fn shutdown_does_not_wait_for_full_ingress() {
    let (hub, guard, _release_dispatcher) = DiagnosticsHub::start_paused_for_test(1);
    hub.publish(vec![pending("queued")]).unwrap();
    hub.publish(vec![pending("dropped")]).unwrap();
    let started = std::time::Instant::now();
    drop(guard);
    assert!(started.elapsed() < Duration::from_millis(500));
}
