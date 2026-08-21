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
fn execution_channel_adapter_serializes_opaque_ids_as_decimal_strings() {
    let unsafe_id = 9_007_199_254_740_993_u64;
    let basis = CompilationBasis {
        graph_revision: crate::node_system::document::GraphRevision::new(unsafe_id),
        registry_fingerprint: RegistryFingerprint::from_bytes([2; 32]),
        resource_versions: Default::default(),
        resource_observations: Default::default(),
    };
    let correlation = CorrelationContext {
        project_session_id: ProjectSessionId::new("session"),
        graph_path: crate::node_system::document::GraphResourcePath("events/main".into()),
        graph_revision: basis.graph_revision,
        registry_fingerprint: basis.registry_fingerprint.clone(),
        resource_versions: basis.resource_versions.clone(),
        compile_id: CompileId::new(unsafe_id),
        selection_digest: Some("demand-selection-a".into()),
        run_id: Some(RunId::new(unsafe_id)),
        node_id: None,
        node_type_id: None,
        parent_call: Some(ParentCallId::new(unsafe_id)),
        trace_parent_span_id: None,
    };
    let operation = execution_channel_event_dto(GraphExecutionStreamEvent::RunEvent(RunEvent {
        correlation: correlation.clone(),
        basis: basis.clone(),
        kind: RunEventKind::OperationStarted {
            operation_index: 3,
            activation_id: unsafe_id,
            attempt_id: unsafe_id,
        },
    }));
    let preview = execution_channel_event_dto(GraphExecutionStreamEvent::RunEvent(RunEvent {
        correlation: CorrelationContext {
            selection_digest: Some("demand-selection-b".into()),
            ..correlation.clone()
        },
        basis: basis.clone(),
        kind: RunEventKind::RunStarted,
    }));

    let result = execution_channel_event_dto(GraphExecutionStreamEvent::RunEvent(RunEvent {
        correlation,
        basis,
        kind: RunEventKind::ResultGroupChanged {
            activation_id: unsafe_id,
            result_ids: vec![ResultId::new(unsafe_id)].into_boxed_slice(),
            state: crate::node_system::runtime::ResultStateKind::Ready,
        },
    }));

    let operation = serde_json::to_value(operation).unwrap();
    assert_eq!(
        operation["correlation"]["graphRevision"],
        unsafe_id.to_string()
    );
    assert_eq!(operation["correlation"]["compileId"], unsafe_id.to_string());
    assert_eq!(
        operation["correlation"]["selectionDigest"],
        "demand-selection-a"
    );
    assert_eq!(operation["correlation"]["runId"], unsafe_id.to_string());
    assert_eq!(
        operation["correlation"]["parentCall"],
        unsafe_id.to_string()
    );
    assert_eq!(operation["basis"]["graphRevision"], unsafe_id.to_string());
    assert_eq!(operation["kind"]["activationId"], unsafe_id.to_string());
    assert!(operation["correlation"].get("graph_revision").is_none());
    assert!(operation["correlation"].get("selection_digest").is_none());
    let preview = serde_json::to_value(preview).unwrap();
    assert_eq!(
        operation["correlation"]["compileId"],
        preview["correlation"]["compileId"]
    );
    assert_ne!(
        operation["correlation"]["selectionDigest"],
        preview["correlation"]["selectionDigest"]
    );

    let result = serde_json::to_value(result).unwrap();
    assert_eq!(
        result["correlation"]["selectionDigest"],
        "demand-selection-a"
    );
    assert_eq!(result["kind"]["activationId"], unsafe_id.to_string());
    assert_eq!(result["kind"]["resultIds"][0], unsafe_id.to_string());
    let execute_result = serde_json::to_value(ExecuteGraphResultDto {
        run_id: unsafe_id.to_string(),
    })
    .unwrap();
    assert_eq!(execute_result["runId"], unsafe_id.to_string());
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
    ));
    let status = execution_channel_event_dto(GraphExecutionStreamEvent::RunOutput(
        RunOutputMessage::Status(crate::node_system::runtime::RunOutputStatusEvent {
            run_id,
            sequence: 2,
            stream: crate::node_system::runtime::RunOutputStream::Stdout,
            status: crate::node_system::runtime::RunOutputStatus::Truncated,
            source_graph_path,
            source_node_id,
        }),
    ));

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
