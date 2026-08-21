use super::*;

#[test]
fn pin_preview_generation_dto_serializes_as_a_safe_number() {
    assert_eq!(
        serde_json::to_value(PinPreviewGenerationDto {
            generation: crate::node_system::plan::MAX_SAFE_PREVIEW_GENERATION,
        })
        .unwrap(),
        serde_json::json!({ "generation": 9_007_199_254_740_991_u64 }),
    );
}

#[test]
fn execution_errors_report_actual_terminal_delivery_and_stable_codes() {
    let no_delivery = GraphExecutionDeliveryReport::default();
    let cancelled_delivery = terminal_delivery_report(
        TerminalRunEventKind::Cancelled,
        DeliveryDisposition::Delivered,
    );
    let failed_delivery = terminal_delivery_report(
        TerminalRunEventKind::Errored,
        DeliveryDisposition::Delivered,
    );
    let cancelled = execution_command_error(
        crate::project::ProjectExecutionError::from(
            crate::node_system::runtime::RunError::Cancelled,
        ),
        &cancelled_delivery,
    );
    let failed = execution_command_error(
        crate::project::ProjectExecutionError::from(
            crate::node_system::runtime::RunError::InvalidPlan("operation failed".into()),
        ),
        &failed_delivery,
    );
    let rejected_terminal = execution_command_error(
        crate::project::ProjectExecutionError::from(
            crate::node_system::runtime::RunError::InvalidPlan("operation failed".into()),
        ),
        &terminal_delivery_report(TerminalRunEventKind::Errored, DeliveryDisposition::Rejected),
    );
    let pre_run = execution_command_error(
        crate::project::ProjectExecutionError::internal("compile failed"),
        &no_delivery,
    );
    let invalid_demand = execution_command_error(
        crate::project::ProjectExecutionError::invalid_demand("requested output node is missing"),
        &no_delivery,
    );
    let recovery_required = execution_command_error(
        crate::project::ProjectExecutionError::recovery_required("project requires recovery"),
        &no_delivery,
    );
    let internal_failure = execution_command_error(
        crate::project::ProjectExecutionError::internal_compilation(
            crate::node_system::compiler::InternalCompilationFailure {
                stage: crate::node_system::compiler::CompilationStage::Lowering,
                code: "compiler.lowering.internal_invariant".into(),
                node_id: Some(crate::node_system::document::NodeId::from_uuid(
                    uuid::Uuid::from_u128(42),
                )),
            },
        ),
        &no_delivery,
    );
    let channel_failure = execution_channel_command_error();

    assert_eq!(cancelled.code(), "run_cancelled");
    assert_eq!(failed.code(), "run_failed");
    assert_eq!(rejected_terminal.code(), "internal_error");
    assert!(rejected_terminal.details().is_none());
    assert_eq!(channel_failure.code(), "execution_channel_failed");
    assert!(channel_failure.details().is_none());
    assert!(channel_failure.incident_id().is_some());
    assert_eq!(pre_run.code(), "internal_error");
    assert_eq!(invalid_demand.code(), "invalid_execution_demand");
    assert_eq!(recovery_required.code(), "project_recovery_required");
    assert_eq!(
        recovery_required.details(),
        serde_json::json!({ "recoveryRequired": true }).as_object(),
    );
    assert!(recovery_required.incident_id().is_none());
    assert_eq!(internal_failure.code(), "internal_compilation_failure");
    assert_eq!(
        internal_failure.details(),
        serde_json::json!({
            "internalCompilationFailure": {
                "stage": "lowering",
                "code": "compiler.lowering.internal_invariant",
                "nodeId": "00000000-0000-0000-0000-00000000002a"
            }
        })
        .as_object(),
    );
    assert_eq!(
        cancelled.details(),
        serde_json::json!({ "terminalRunEventSent": true }).as_object(),
    );
    assert_eq!(
        failed.details(),
        serde_json::json!({ "terminalRunEventSent": true }).as_object(),
    );
    assert!(pre_run.details().is_none());
    assert!(failed.incident_id().is_some());
    assert!(pre_run.incident_id().is_some());
}

#[test]
fn relational_execution_errors_keep_exact_command_codes() {
    for (relational, expected_code) in [
        (
            crate::node_system::runtime::RelationalErrorCode::HintInvalid,
            "relational_hint_invalid",
        ),
        (
            crate::node_system::runtime::RelationalErrorCode::TypeMismatch,
            "relational_type_mismatch",
        ),
    ] {
        let error = crate::project::ProjectExecutionError::from(
            crate::node_system::runtime::RunError::RelationalFailed {
                operation: crate::node_system::plan::OperationIndex::new(2),
                code: relational,
                message: "sensitive detail".into(),
            },
        );

        let mapped = execution_command_error(
            error,
            &terminal_delivery_report(
                TerminalRunEventKind::Errored,
                DeliveryDisposition::Delivered,
            ),
        );

        assert_eq!(mapped.code(), expected_code);
        assert_eq!(
            mapped.details(),
            serde_json::json!({ "terminalRunEventSent": true }).as_object(),
        );
        let wire = serde_json::to_string(&mapped).unwrap();
        assert!(!wire.contains("message"));
        assert!(!wire.contains("sensitive detail"));
        assert!(mapped.incident_id().is_some());
    }
}

#[test]
fn execution_channel_adapter_serializes_minimal_run_and_preview_ids() {
    let unsafe_id = 9_007_199_254_740_993_u64;
    let run = GraphRunIdentity {
        project_session_id: ProjectSessionId::new("session"),
        graph_path: crate::node_system::document::GraphResourcePath(
            "events/Main.yssbi-event".into(),
        ),
        run_id: RunId::new(unsafe_id),
    };
    let started = execution_channel_event_dto(GraphExecutionStreamEvent::RunEvent(RunEvent {
        run: run.clone(),
        kind: RunEventKind::RunStarted,
    }))
    .unwrap();
    let output = crate::node_system::plan::GraphOutputRef {
        graph_path: run.graph_path.clone(),
        port: crate::node_system::document::PortAddress::declared(
            NodeId::from_uuid(uuid::Uuid::from_u128(2)),
            crate::node_system::protocol::PortKey::new("result").unwrap(),
        ),
    };
    let preview = execution_channel_event_dto(GraphExecutionStreamEvent::RunEvent(RunEvent {
        run: run.clone(),
        kind: RunEventKind::PinPreviewResultReady {
            output: output.clone(),
            generation: crate::node_system::plan::MAX_SAFE_PREVIEW_GENERATION,
            result_id: ResultId::new(unsafe_id),
        },
    }))
    .unwrap();

    assert_eq!(
        serde_json::to_value(started).unwrap(),
        serde_json::json!({
            "run": {
                "projectSessionId": "session",
                "graphPath": "events/Main.yssbi-event",
                "runId": unsafe_id.to_string(),
            },
            "kind": { "type": "runStarted" },
        })
    );
    let preview = serde_json::to_value(preview).unwrap();
    assert_eq!(preview["run"]["runId"], unsafe_id.to_string());
    assert_eq!(preview["kind"]["resultId"], unsafe_id.to_string());
    assert_eq!(
        preview["kind"]["generation"],
        crate::node_system::plan::MAX_SAFE_PREVIEW_GENERATION,
    );
    assert!(matches!(
        execution_channel_event_dto(GraphExecutionStreamEvent::RunEvent(RunEvent {
            run,
            kind: RunEventKind::PinPreviewResultReady {
                output,
                generation: crate::node_system::plan::MAX_SAFE_PREVIEW_GENERATION + 1,
                result_id: ResultId::new(unsafe_id),
            },
        })),
        Err(crate::commands::node_system_execution_dto::RunEventDtoError::UnsafePreviewGeneration)
    ));
    assert_eq!(
        parse_opaque_u64("resultId", &unsafe_id.to_string()).unwrap(),
        unsafe_id,
    );
    for invalid in ["not-decimal", "0", "01", "+1", "-1"] {
        assert_eq!(
            parse_opaque_u64("runId", invalid).unwrap_err().code(),
            "invalid_opaque_id",
        );
    }
}

#[test]
fn run_output_channel_adapter_uses_a_separate_exact_wire_shape() {
    let run_id = RunId::new(9_007_199_254_740_993);
    let source_graph_path =
        crate::node_system::document::GraphResourcePath("functions/output.yssbi-function".into());
    let source_node_id = NodeId::from_uuid(uuid::Uuid::from_u128(2));
    let output = execution_channel_event_dto(GraphExecutionStreamEvent::RunOutput(
        RunOutputMessage::Output(crate::node_system::runtime::RunOutputEvent {
            run_id,
            sequence: 1,
            stream: crate::node_system::runtime::RunOutputStream::Stdout,
            text: "user-visible value".into(),
            source_graph_path: source_graph_path.clone(),
            source_node_id,
        }),
    ))
    .unwrap();
    let status = execution_channel_event_dto(GraphExecutionStreamEvent::RunOutput(
        RunOutputMessage::Status(crate::node_system::runtime::RunOutputStatusEvent {
            run_id,
            sequence: 2,
            stream: crate::node_system::runtime::RunOutputStream::Stdout,
            status: crate::node_system::runtime::RunOutputStatus::Truncated,
            source_graph_path,
            source_node_id,
        }),
    ))
    .unwrap();

    assert_eq!(
        serde_json::to_value(output).unwrap(),
        serde_json::json!({
            "runId": "9007199254740993",
            "sequence": 1,
            "stream": "stdout",
            "text": "user-visible value",
            "sourceGraphPath": "functions/output.yssbi-function",
            "sourceNodeId": "00000000-0000-0000-0000-000000000002",
        })
    );
    assert_eq!(
        serde_json::to_value(status).unwrap(),
        serde_json::json!({
            "runId": "9007199254740993",
            "sequence": 2,
            "stream": "stdout",
            "status": "truncated",
            "sourceGraphPath": "functions/output.yssbi-function",
            "sourceNodeId": "00000000-0000-0000-0000-000000000002",
        })
    );
}
