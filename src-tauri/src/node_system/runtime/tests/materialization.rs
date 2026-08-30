use super::*;

#[test]
fn bounded_materialization_capacity_one_applies_backpressure() {
    let root = materialization_test_root("backpressure");
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(1),
        materialization_test_budgets(1, 1024),
        CancellationToken::new(),
        root.clone(),
    )
    .unwrap();
    let (observed_tx, observed_rx) = mpsc::channel();
    let values = (0..3).map(move |value| {
        observed_tx.send(value).unwrap();
        Value::Integer(value)
    });

    let stream = owner.stream_from_values(values).unwrap();

    assert_eq!(observed_rx.recv_timeout(Duration::from_secs(1)).unwrap(), 0);
    assert_eq!(observed_rx.recv_timeout(Duration::from_secs(1)).unwrap(), 1);
    assert!(observed_rx.recv_timeout(Duration::from_millis(50)).is_err());
    assert_eq!(stream.recv().unwrap(), Value::Integer(0));
    assert_eq!(observed_rx.recv_timeout(Duration::from_secs(1)).unwrap(), 2);
    assert_eq!(stream.recv().unwrap(), Value::Integer(1));
    assert_eq!(stream.recv().unwrap(), Value::Integer(2));
    assert_eq!(stream.recv(), Err(StreamReceiveError::Closed));

    drop(owner);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_producer_panic_is_not_partial_success() {
    let root = materialization_test_root("producer-panic");
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(5),
        materialization_test_budgets(1, 1024),
        CancellationToken::new(),
        root.clone(),
    )
    .unwrap();
    let stream = owner
        .stream_from_values((0..2).map(|value| {
            if value == 1 {
                panic!("producer iterator panic sentinel");
            }
            Value::Integer(value)
        }))
        .unwrap();

    assert_eq!(stream.recv().unwrap(), Value::Integer(0));
    assert!(matches!(
        stream.recv(),
        Err(StreamReceiveError::Failed(message))
            if message.as_ref() == "stream producer panicked"
    ));

    drop(stream);
    drop(owner);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn collect_adapter_uses_pending_value_writer() {
    let root = materialization_test_root("writer-integration");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(201),
        materialization_test_budgets(1, 1024),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();

    execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 10,
                max_bytes: 1024,
            },
        },
        RuntimeValue::Artifact(Artifact::new(
            ArtifactKind::Buffered,
            [Value::Integer(1), Value::Integer(2)],
        )),
        &owner,
        &cancellation,
    )
    .unwrap();

    assert_eq!(owner.pending_writer_count_for_test(), 1);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn kernel_unreserved_artifact_respects_owner_memory_budget() {
    let root = materialization_test_root("kernel-artifact-budget");
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("unreserved_artifact", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| {
                Ok(vec![RuntimeValue::Artifact(Artifact::new(
                    ArtifactKind::Collected,
                    [Value::String("larger than one byte".into())],
                ))])
            }),
        )
        .unwrap();
    let output = stable_output("unreserved");
    let mut source = operation("unreserved_artifact", &[], &[0]);
    source.outputs[0].public_output = Some(output.clone());
    let execution_plan = plan(
        vec![source],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let results = ResultStore::new();

    RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        results.clone(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_result_store(&results)
    .with_resource_budgets(materialization_test_budgets(1, 1))
    .with_test_spill_root(root.clone())
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    let result_id = results.pin_history(&output)[0].result_id;
    let result = results.result(result_id).unwrap();
    let ResultState::Ready(value) = &result.state else {
        panic!("kernel artifact result must be ready");
    };
    assert!(value.is_spill_backed());
    drop(result);
    drop(results);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spill_memory_threshold_preserves_stable_disk_order() {
    let root = materialization_test_root("spill-order");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(2),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let input = RuntimeValue::Artifact(Artifact::new(
        ArtifactKind::Collected,
        [
            Value::Integer(3),
            Value::String("two".into()),
            Value::Bool(true),
        ],
    ));

    let output = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 10,
                max_bytes: 1024,
            },
        },
        input,
        &owner,
        &cancellation,
    )
    .unwrap();

    let RuntimeValue::Artifact(artifact) = output else {
        panic!("collect must produce an artifact");
    };
    assert!(matches!(
        artifact.materialized(),
        MaterializedArtifact::Spilled(_)
    ));
    assert_eq!(
        artifact
            .cursor()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        [
            Value::Integer(3),
            Value::String("two".into()),
            Value::Bool(true)
        ]
    );
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());

    drop(artifact);
    drop(owner);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spilled_artifact_exposes_only_explicit_non_panicking_consumption() {
    let root = materialization_test_root("explicit-artifact-consumption");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(13),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let spilled = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Scalar(Value::Integer(7)),
        &owner,
        &cancellation,
    )
    .unwrap();
    let RuntimeValue::Artifact(artifact) = spilled else {
        panic!("spill adapter must return an artifact");
    };

    assert_eq!(artifact.in_memory_values(), None);
    assert_eq!(
        artifact
            .cursor()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        [Value::Integer(7)]
    );

    drop(artifact);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spill_backed_adapter_value_supports_two_independent_passes() {
    let root = materialization_test_root("independent-readers");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(3),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let spilled = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Artifact(Artifact::new(
            ArtifactKind::Buffered,
            [Value::Integer(1), Value::Integer(2), Value::Integer(3)],
        )),
        &owner,
        &cancellation,
    )
    .unwrap();
    let RuntimeValue::Artifact(artifact) = spilled else {
        panic!("reusable storage must remain an artifact kernel view");
    };
    let stored = artifact.into_stored_value();

    let first = stored.open_reader().unwrap();
    let second = stored.open_reader().unwrap();
    assert_eq!(
        first.collect::<Result<Vec<_>, _>>().unwrap(),
        [Value::Integer(1), Value::Integer(2), Value::Integer(3),]
    );
    assert_eq!(
        second.collect::<Result<Vec<_>, _>>().unwrap(),
        [Value::Integer(1), Value::Integer(2), Value::Integer(3),]
    );

    drop(stored);
    drop(owner);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_relational_ingress_consumes_spilled_single_value() {
    let root = materialization_test_root("relational-consumer");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(6),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let dataframe = Value::Object(BTreeMap::from([(
        Box::<str>::from("value"),
        Value::List(vec![Value::Integer(1), Value::Integer(2)]),
    )]));
    let spilled = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 10,
                max_bytes: 1024 * 1024,
            },
        },
        RuntimeValue::Artifact(Artifact::new(ArtifactKind::Buffered, [dataframe])),
        &owner,
        &cancellation,
    )
    .unwrap();

    let converted =
        super::super::relational_dataframe::tabular_runtime_to_dataframe(spilled).unwrap();

    assert_eq!(converted.height(), 2);
    assert_eq!(converted.width(), 1);
    drop(owner);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_memory_exact_boundary_and_drop_release() {
    let value = Value::String("exact".into());
    let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
    let root = materialization_test_root("memory-exact");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(7),
        materialization_test_budgets(1, bytes),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();

    let artifact = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 1,
                max_bytes: bytes,
            },
        },
        RuntimeValue::Scalar(value),
        &owner,
        &cancellation,
    )
    .unwrap();

    assert!(matches!(
        artifact,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::InMemory(_))
    ));
    assert_eq!(owner.memory_bytes_for_test(), bytes);
    drop(artifact);
    assert_eq!(owner.memory_bytes_for_test(), 0);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_clone_shares_values_and_one_live_reservation() {
    let value = Value::String("shared-clone".into());
    let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
    let root = materialization_test_root("memory-clone-sharing");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(16),
        materialization_test_budgets(1, bytes),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let RuntimeValue::Artifact(artifact) = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 1,
                max_bytes: bytes,
            },
        },
        RuntimeValue::Scalar(value),
        &owner,
        &cancellation,
    )
    .unwrap() else {
        panic!("collect must return an artifact");
    };

    let cloned = artifact.clone();
    assert!(std::ptr::eq(
        artifact.in_memory_values().unwrap().as_ptr(),
        cloned.in_memory_values().unwrap().as_ptr(),
    ));
    assert_eq!(owner.memory_bytes_for_test(), bytes);
    drop(artifact);
    assert_eq!(owner.memory_bytes_for_test(), bytes);
    drop(cloned);
    assert_eq!(owner.memory_bytes_for_test(), 0);

    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_adapter_consumption_retains_source_reservation() {
    let value = Value::String("adapter-handoff".into());
    let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
    let root = materialization_test_root("memory-adapter-handoff");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(17),
        materialization_test_budgets(1, bytes),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let collect = PlannedAdapter::Collect {
        limits: MaterializationLimits {
            max_values: 1,
            max_bytes: bytes,
        },
    };
    let first =
        execute_planned_adapter(&collect, RuntimeValue::Scalar(value), &owner, &cancellation)
            .unwrap();
    assert_eq!(owner.memory_bytes_for_test(), bytes);

    let second = execute_planned_adapter(&collect, first, &owner, &cancellation).unwrap();

    assert!(matches!(
        second,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::Spilled(_))
    ));
    assert_eq!(owner.memory_bytes_for_test(), 0);
    drop(second);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_aggregate_memory_spills_then_reuses_released_capacity() {
    let value = Value::String("aggregate".into());
    let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
    let root = materialization_test_root("memory-aggregate");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(8),
        materialization_test_budgets(1, bytes),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let collect = |value| {
        execute_planned_adapter(
            &PlannedAdapter::Collect {
                limits: MaterializationLimits {
                    max_values: 1,
                    max_bytes: bytes,
                },
            },
            RuntimeValue::Scalar(value),
            &owner,
            &cancellation,
        )
        .unwrap()
    };

    let first = collect(value.clone());
    let second = collect(value.clone());
    assert!(matches!(
        second,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::Spilled(_))
    ));
    assert_eq!(owner.memory_bytes_for_test(), bytes);
    drop(first);
    assert_eq!(owner.memory_bytes_for_test(), 0);
    let third = collect(value);
    assert!(matches!(
        third,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::InMemory(_))
    ));
    drop(second);
    drop(third);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_single_oversized_value_spills_without_reserving_memory() {
    let oversized = Value::Bytes(vec![7; 4096]);
    let small = Value::Bool(true);
    let small_bytes = serde_json::to_vec(&small).unwrap().len() as u64;
    let root = materialization_test_root("memory-oversized");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(15),
        materialization_test_budgets(1, small_bytes),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();

    let oversized = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 1,
                max_bytes: 64 * 1024,
            },
        },
        RuntimeValue::Scalar(oversized),
        &owner,
        &cancellation,
    )
    .unwrap();
    assert!(matches!(
        oversized,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::Spilled(_))
    ));
    assert_eq!(owner.memory_bytes_for_test(), 0);

    let subsequent = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 1,
                max_bytes: small_bytes,
            },
        },
        RuntimeValue::Scalar(small),
        &owner,
        &cancellation,
    )
    .unwrap();
    assert!(matches!(
        subsequent,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::InMemory(_))
    ));
    assert_eq!(owner.memory_bytes_for_test(), small_bytes);

    drop(oversized);
    drop(subsequent);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_limit_failure_rolls_back_memory_for_next_allocation() {
    let value = Value::String("rollback".into());
    let bytes = serde_json::to_vec(&value).unwrap().len() as u64;
    let root = materialization_test_root("memory-rollback");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(9),
        materialization_test_budgets(1, bytes),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();

    let failed = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 0,
                max_bytes: bytes,
            },
        },
        RuntimeValue::Scalar(value.clone()),
        &owner,
        &cancellation,
    );
    assert!(failed.is_err());
    assert_eq!(owner.memory_bytes_for_test(), 0);

    let next = execute_planned_adapter(
        &PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 1,
                max_bytes: bytes,
            },
        },
        RuntimeValue::Scalar(value),
        &owner,
        &cancellation,
    )
    .unwrap();
    assert!(matches!(
        next,
        RuntimeValue::Artifact(ref artifact)
            if matches!(artifact.materialized(), MaterializedArtifact::InMemory(_))
    ));
    drop(next);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spill_typed_fidelity_covers_all_value_variants_and_nested_data() {
    use crate::graph::protocol::CanonicalDecimal;

    let values = vec![
        Value::Null,
        Value::Bool(true),
        Value::Integer(-7),
        Value::Unsigned(9),
        Value::Decimal(CanonicalDecimal::new("12.34").unwrap()),
        Value::String("text".into()),
        Value::Bytes(vec![0, 1, 255]),
        Value::List(vec![Value::Integer(1), Value::String("nested".into())]),
        Value::Object(BTreeMap::from([(
            Box::<str>::from("nested"),
            Value::List(vec![Value::Bool(false), Value::Null]),
        )])),
    ];
    let root = materialization_test_root("typed-fidelity");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(10),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();

    let spilled = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Artifact(Artifact::new(ArtifactKind::Buffered, values.clone())),
        &owner,
        &cancellation,
    )
    .unwrap();
    let RuntimeValue::Artifact(artifact) = spilled else {
        panic!("spill adapter must return an artifact");
    };
    assert_eq!(
        artifact
            .cursor()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        values
    );
    drop(artifact);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spill_cursor_keeps_promoted_file_alive_until_cursor_drop() {
    let root = materialization_test_root("spill-cursor-lifetime");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(18),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let RuntimeValue::Artifact(artifact) = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Scalar(Value::Integer(1)),
        &owner,
        &cancellation,
    )
    .unwrap() else {
        panic!("spill adapter must return an artifact");
    };
    artifact.promote(&cancellation, None).unwrap();
    let MaterializedArtifact::Spilled(spill) = artifact.materialized() else {
        panic!("spill adapter must use spill storage");
    };
    let path = spill.path_for_test();
    let cursor = spill.cursor().unwrap();

    drop(artifact);
    assert!(path.exists());
    drop(cursor);
    assert!(!path.exists());

    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn deadline_during_spill_promotion_publishes_no_durable_artifact() {
    let root = materialization_test_root("spill-promotion-deadline");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(20),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let staged_path = root.join("pending-promotion.jsonf");
    let metadata = super::super::spill::write_spill(
        &staged_path,
        std::iter::once(Ok(Value::Integer(1))),
        &cancellation,
        |_| Ok(()),
    )
    .unwrap();
    let spill = Arc::new(super::super::spill::SpillStorage::new(
        staged_path.clone(),
        metadata,
        ArtifactValueKind::Sequence,
        None,
        [0; 32],
        None,
    ));
    let artifact = Artifact::from_stored_value(
        ArtifactKind::Spilled,
        StoredValue::spill_backed(Arc::clone(&spill)),
    );
    spill.set_promotion_checkpoint_for_test(Arc::new(|| {
        thread::sleep(Duration::from_millis(20));
    }));

    assert_eq!(
        artifact.promote(
            &cancellation,
            Some(RunDeadline::after(Duration::from_millis(5))),
        ),
        Err(RunError::DeadlineExceeded {
            phase: RunPhase::ResultPublication,
        })
    );
    assert_eq!(spill.path_for_test(), staged_path);
    assert!(!staged_path.exists());

    drop(artifact);
    drop(owner);
    let _ = std::fs::remove_dir(root);
}

#[test]
fn spill_cursor_close_surfaces_and_retries_transient_delete_failure() {
    let root = materialization_test_root("spill-cursor-delete-retry");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(19),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let RuntimeValue::Artifact(artifact) = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Scalar(Value::Integer(2)),
        &owner,
        &cancellation,
    )
    .unwrap() else {
        panic!("spill adapter must return an artifact");
    };
    artifact.promote(&cancellation, None).unwrap();
    let MaterializedArtifact::Spilled(spill) = artifact.materialized() else {
        panic!("spill adapter must use spill storage");
    };
    spill.fail_next_deletions_for_test(1);
    let path = spill.path_for_test();
    let mut cursor = spill.cursor().unwrap();
    drop(artifact);

    let error = cursor.close().unwrap_err();

    assert!(
        error
            .to_string()
            .contains("injected spill deletion failure")
    );
    assert!(path.exists());
    cursor.close().unwrap();
    assert!(!path.exists());
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spill_quota_exact_boundary_and_failure_rollback_allow_subsequent_write() {
    let first = Value::String("first".into());
    let second = Value::String("second".into());
    let first_bytes = 8 + serde_json::to_vec(&first).unwrap().len() as u64;
    let second_bytes = 8 + serde_json::to_vec(&second).unwrap().len() as u64;
    let root = materialization_test_root("spill-quota");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(11),
        RunResourceBudgets {
            stream_capacity: std::num::NonZeroUsize::new(1).unwrap(),
            materialization_memory_bytes: 1,
            spill_directory_bytes: first_bytes,
        },
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();

    let failed = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Artifact(Artifact::new(
            ArtifactKind::Buffered,
            [first.clone(), second],
        )),
        &owner,
        &cancellation,
    );
    assert!(failed.is_err());
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());

    let exact = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Scalar(first),
        &owner,
        &cancellation,
    )
    .unwrap();
    assert!(matches!(exact, RuntimeValue::Artifact(_)));
    assert_eq!(owner.spill_bytes_for_test(), first_bytes);
    assert!(second_bytes > 0);
    drop(exact);
    assert_eq!(owner.spill_bytes_for_test(), 0);

    let again = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Scalar(Value::String("first".into())),
        &owner,
        &cancellation,
    )
    .unwrap();
    assert_eq!(owner.spill_bytes_for_test(), first_bytes);
    drop(again);
    assert_eq!(owner.spill_bytes_for_test(), 0);
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn bounded_materialization_producer_registration_is_linearized_with_cleanup() {
    let root = materialization_test_root("producer-race");
    let owner = Arc::new(
        RunResourceOwner::with_spill_root(
            RunId::new(12),
            materialization_test_budgets(1, 1024),
            CancellationToken::new(),
            root.clone(),
        )
        .unwrap(),
    );
    let registration_reached = Arc::new(Barrier::new(2));
    let release_registration = Arc::new(Barrier::new(2));
    let producer = {
        let owner = Arc::clone(&owner);
        let registration_reached = Arc::clone(&registration_reached);
        let release_registration = Arc::clone(&release_registration);
        thread::spawn(move || {
            owner.stream_from_values_with_registration_checkpoint([Value::Integer(1)], move || {
                registration_reached.wait();
                release_registration.wait();
            })
        })
    };
    registration_reached.wait();
    let (cleanup_done_tx, cleanup_done_rx) = mpsc::channel();
    let cleanup = {
        let owner = Arc::clone(&owner);
        thread::spawn(move || {
            let errors = owner.cleanup();
            cleanup_done_tx.send(errors).unwrap();
        })
    };
    assert!(
        cleanup_done_rx
            .recv_timeout(Duration::from_millis(50))
            .is_err()
    );
    release_registration.wait();
    let stream = producer.join().unwrap().unwrap();
    drop(stream);
    assert!(
        cleanup_done_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .is_empty()
    );
    cleanup.join().unwrap();
    assert!(owner.stream_from_values([Value::Integer(2)]).is_err());
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    drop(owner);
    std::fs::remove_dir(root).unwrap();
}

struct KernelOwnedStreamSource;

impl Kernel for KernelOwnedStreamSource {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        Ok(vec![RuntimeValue::Stream(
            context
                .resource_owner
                .stream_from_values([Value::Integer(1), Value::Integer(2)])
                .map_err(|error| KernelError::new(error.to_string()))?,
        )])
    }
}

#[test]
fn bounded_materialization_kernel_streams_use_the_scheduler_owner() {
    let root = materialization_test_root("kernel-owner");
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("kernel_owned_stream_source", KernelHandle::new),
            KernelOwnedStreamSource,
        )
        .unwrap();
    kernels
        .register(
            id("kernel_owned_stream_sink", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                let RuntimeValue::Artifact(artifact) = &inputs[0] else {
                    return Err(KernelError::new("expected collected stream"));
                };
                Ok(vec![
                    Value::Integer(artifact.cursor().unwrap().count() as i64).into(),
                ])
            }),
        )
        .unwrap();
    let mut source = operation("kernel_owned_stream_source", &[], &[0]);
    source.outputs[0].production = OutputProduction::Streaming;
    let collect = adapter_operation(
        "kernel.owner.collect",
        1,
        2,
        OutputProduction::Streaming,
        InputConsumption::FullyMaterialized,
    );
    let sink = operation("kernel_owned_stream_sink", &[3], &[4]);
    let mut execution_plan = plan(
        vec![source, collect, sink],
        5,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
            ControlStep::Operation(OperationIndex::new(2)),
        ])),
    );
    execution_plan.value_dependencies = Box::new([
        ValueDependency {
            source: ValueRef::new(0),
            destination: ValueRef::new(1),
        },
        ValueDependency {
            source: ValueRef::new(2),
            destination: ValueRef::new(3),
        },
    ]);
    execution_plan.results = Box::new([PlanResult {
        name: "count".into(),
        output: stable_output("count"),
        value: ValueRef::new(4),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_resource_budgets(materialization_test_budgets(1, 1024))
    .with_test_spill_root(root.clone())
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(
        result.value_for_test("count").unwrap(),
        Value::Integer(2).into()
    );
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

struct PanicWithCleanupFailureKernel;

impl Kernel for PanicWithCleanupFailureKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        context
            .resource_owner
            .register_panicking_cleanup_task_for_test();
        panic!("primary kernel panic sentinel")
    }
}

#[test]
fn bounded_materialization_panic_cleanup_failure_is_diagnosed_without_replacing_panic() {
    use tracing_subscriber::layer::SubscriberExt;

    const PRIMARY_PANIC_PAYLOAD: &str = "primary kernel panic sentinel";
    const CLEANUP_PANIC_PAYLOAD: &str = "cleanup task panic sentinel";

    let root = materialization_test_root("panic-diagnostic");
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("panic_with_cleanup_failure", KernelHandle::new),
            PanicWithCleanupFailureKernel,
        )
        .unwrap();
    let execution_plan = plan(
        vec![operation("panic_with_cleanup_failure", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    let resources = no_resources();
    let functions = NoFunctions;
    let executor = RunExecutor::new(
        &kernels,
        &resources,
        &functions,
        ResultStore::new(),
        Arc::new(SessionMemoization::new()),
    )
    .with_test_spill_root(root.clone());
    let diagnostics = yss_diagnostics::DiagnosticsRuntime::initialize().unwrap();
    let subscriber = tracing_subscriber::registry()
        .with(yss_tracing::LogLayer::new(diagnostics.rust_log_sink()));

    let panic = tracing::subscriber::with_default(subscriber, || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = executor.run(&execution_plan, CancellationToken::new());
        }))
    });

    let payload = panic.expect_err("the primary kernel panic must propagate");
    assert_eq!(
        payload.downcast_ref::<&str>().copied(),
        Some(PRIMARY_PANIC_PAYLOAD)
    );
    let subscription = diagnostics.subscribe_batches(|_| true).unwrap();
    let cleanup_diagnostics = subscription
        .entries
        .iter()
        .filter(|record| record.event.as_deref() == Some("resourceCleanupFailed"))
        .collect::<Vec<_>>();
    assert_eq!(cleanup_diagnostics.len(), 1);
    let cleanup = cleanup_diagnostics[0];
    assert_eq!(cleanup.level, yss_diagnostics::DiagnosticLevel::Warn);
    assert_eq!(
        cleanup.domain,
        yss_diagnostics::DiagnosticDomain::Execution
    );
    assert_eq!(cleanup.target, "yssbi::node_system::runtime::cleanup");
    assert_eq!(cleanup.message, "Runtime resource cleanup failed");
    assert_eq!(cleanup.fields["error"], "stream producer panicked");
    let encoded = serde_json::to_string(cleanup).unwrap();
    assert!(!encoded.contains(PRIMARY_PANIC_PAYLOAD));
    assert!(!encoded.contains(CLEANUP_PANIC_PAYLOAD));
    diagnostics
        .unsubscribe(subscription.subscription_id)
        .unwrap();

    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

fn spilling_terminal_plan(terminal: PlannedOperation) -> ExecutionPlan {
    let mut source = operation("spill_terminal_source", &[], &[0]);
    source.outputs[0].production = OutputProduction::Streaming;
    let collect = adapter_operation(
        "spill.terminal.collect",
        1,
        2,
        OutputProduction::Streaming,
        InputConsumption::FullyMaterialized,
    );
    let mut execution_plan = plan(
        vec![source, collect, terminal],
        5,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
            ControlStep::Operation(OperationIndex::new(2)),
        ])),
    );
    execution_plan.value_dependencies = Box::new([
        ValueDependency {
            source: ValueRef::new(0),
            destination: ValueRef::new(1),
        },
        ValueDependency {
            source: ValueRef::new(2),
            destination: ValueRef::new(3),
        },
    ]);
    execution_plan
}

fn spilling_terminal_kernels(terminal: impl Kernel + 'static) -> KernelRegistry {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("spill_terminal_source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::String("spill me".into()).into()])),
        )
        .unwrap();
    kernels
        .register(id("spill_terminal", KernelHandle::new), terminal)
        .unwrap();
    kernels
}

fn spilling_terminal_operation() -> PlannedOperation {
    operation("spill_terminal", &[3], &[4])
}

#[test]
fn bounded_materialization_cleanup_covers_success_error_cancel_and_deadline() {
    enum Terminal {
        Success,
        Error,
        Cancel,
        Deadline,
    }

    for terminal in [
        Terminal::Success,
        Terminal::Error,
        Terminal::Cancel,
        Terminal::Deadline,
    ] {
        let root = materialization_test_root("terminal");
        let (kernels, expected, deadline) = match terminal {
            Terminal::Success => (
                spilling_terminal_kernels(FnKernel(|_: &[RuntimeValue]| {
                    Ok(vec![Value::Integer(1).into()])
                })),
                None,
                false,
            ),
            Terminal::Deadline => (
                spilling_terminal_kernels(FnKernel(|_: &[RuntimeValue]| {
                    Ok(vec![Value::Integer(1).into()])
                })),
                Some(RunErrorCode::Cancelled),
                true,
            ),
            Terminal::Error => (
                spilling_terminal_kernels(ErrorKernel {
                    cancel_token: false,
                    cancelled_error: false,
                }),
                Some(RunErrorCode::KernelFailed),
                false,
            ),
            Terminal::Cancel => (
                spilling_terminal_kernels(ErrorKernel {
                    cancel_token: true,
                    cancelled_error: true,
                }),
                Some(RunErrorCode::Cancelled),
                false,
            ),
        };
        let resources = no_resources();
        let functions = NoFunctions;
        let mut executor = RunExecutor::new(
            &kernels,
            &resources,
            &functions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .with_resource_budgets(materialization_test_budgets(1, 1))
        .with_test_spill_root(root.clone());
        if deadline {
            executor = executor.with_test_checkpoint(Arc::new(|checkpoint, cancellation| {
                if checkpoint == SchedulerCheckpoint::FinalResultPublication {
                    cancellation.cancel();
                }
            }));
        }
        let execution_plan = spilling_terminal_plan(spilling_terminal_operation());
        assert!(
            execution_plan.validate().is_ok(),
            "{:?}",
            execution_plan.validate()
        );
        let result = executor.run(&execution_plan, CancellationToken::new());
        match expected {
            None => assert!(result.is_ok(), "{result:?}"),
            Some(code) => assert_eq!(RunErrorCode::from(&result.unwrap_err()), code),
        }
        assert!(std::fs::read_dir(&root).unwrap().next().is_none());
        std::fs::remove_dir(root).unwrap();
    }
}

#[test]
fn bounded_materialization_cleanup_runs_during_panic_unwind() {
    let root = materialization_test_root("panic");
    let kernels = spilling_terminal_kernels(FnKernel(|_: &[RuntimeValue]| {
        panic!("spill terminal panic sentinel")
    }));
    let resources = no_resources();
    let functions = NoFunctions;
    let executor = RunExecutor::new(
        &kernels,
        &resources,
        &functions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_resource_budgets(materialization_test_budgets(1, 1))
    .with_test_spill_root(root.clone());

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = executor.run(
            &spilling_terminal_plan(spilling_terminal_operation()),
            CancellationToken::new(),
        );
    }));

    assert!(result.is_err());
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

struct ProjectReplacementKernel {
    started: mpsc::Sender<()>,
}

impl Kernel for ProjectReplacementKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.started.send(()).unwrap();
        while !context.cancellation.is_cancelled() {
            thread::yield_now();
        }
        Err(KernelError::cancelled("project replacement cancelled run"))
    }
}

#[test]
fn bounded_materialization_cleanup_precedes_project_replacement_drain_completion() {
    let root = materialization_test_root("replacement");
    let registry = Arc::new(ProjectRunRegistry::new());
    let (started_tx, started_rx) = mpsc::channel();
    let kernels = spilling_terminal_kernels(ProjectReplacementKernel {
        started: started_tx,
    });
    let execution_plan = spilling_terminal_plan(spilling_terminal_operation());
    let project = execution_plan.provenance.project_session_id.clone();
    let run_registry = Arc::clone(&registry);
    let run_root = root.clone();
    let run = thread::spawn(move || {
        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .with_run_registry(&run_registry)
        .with_resource_budgets(materialization_test_budgets(1, 1))
        .with_test_spill_root(run_root)
        .run(&execution_plan, CancellationToken::new())
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();

    registry.cancel_and_drain(&project);

    assert_eq!(run.join().unwrap(), Err(RunError::Cancelled));
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}
