use super::*;

#[test]
fn data_series_artifact_survives_memoization() {
    let root = materialization_test_root("data-series-spill-memo");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(130),
        materialization_test_budgets(1, 1),
        cancellation.clone(),
        root.clone(),
    )
    .unwrap();
    let metadata = DataSeriesMetadata {
        element_type: DataSeriesElementType::Float64,
        length: 3,
        null_count: 1,
        name: Some("spill-backed".into()),
        format: Some("number".into()),
    };
    let series = Artifact::new_data_series(
        ArtifactKind::Collected,
        metadata.clone(),
        [decimal("1"), Value::Null, decimal("3")],
    )
    .unwrap();
    let spilled = execute_planned_adapter(
        &PlannedAdapter::Spill {
            memory_limit_bytes: 1,
        },
        RuntimeValue::Artifact(series),
        &owner,
        &cancellation,
    )
    .unwrap();
    assert!(matches!(
        &spilled,
        RuntimeValue::Artifact(artifact)
            if artifact.kind() == ArtifactKind::Spilled
                && artifact.data_series_metadata() == Some(&metadata)
    ));
    let RuntimeValue::Artifact(spilled_artifact) = &spilled else {
        unreachable!();
    };
    assert!(
        ValueFingerprint::from_stored_value(&spilled_artifact.clone().into_stored_value())
            == ValueFingerprint::from_stored_value(&spilled_artifact.clone().into_stored_value(),),
        "spill-backed logical contents and metadata are fingerprintable"
    );

    let calls = AtomicUsize::new(0);
    let mut outputs = Vec::new();
    for _ in 0..2 {
        calls.fetch_add(1, Ordering::SeqCst);
        let RuntimeValue::Artifact(artifact) = &spilled else {
            unreachable!();
        };
        let values = artifact
            .cursor()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        outputs.push(
            DataSeriesBuilder::new(metadata.element_type)
                .values(values)
                .name(metadata.name.clone().unwrap())
                .format(metadata.format.clone().unwrap())
                .build(ArtifactKind::Collected)
                .unwrap(),
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    for output in outputs {
        assert_eq!(output.data_series_metadata(), Some(&metadata));
        assert_eq!(
            output
                .cursor()
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            [decimal("1"), Value::Null, decimal("3")]
        );
    }

    drop(spilled);
    drop(owner);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn spill_artifacts_enter_session_memoization_by_logical_value() {
    let root = materialization_test_root("spill-memo");
    let cancellation = CancellationToken::new();
    let owner = RunResourceOwner::with_spill_root(
        RunId::new(4),
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
            [Value::Integer(1), Value::Integer(2)],
        )),
        &owner,
        &cancellation,
    )
    .unwrap();
    let RuntimeValue::Artifact(spilled_artifact) = &spilled else {
        unreachable!();
    };
    let first = ValueFingerprint::from_stored_value(&spilled_artifact.clone().into_stored_value());
    let second = ValueFingerprint::from_stored_value(&StoredValue::sequence(
        vec![Value::Integer(1), Value::Integer(2)].into_boxed_slice(),
    ));
    assert_eq!(first, second);

    drop(spilled);
    drop(owner);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}

fn per_run_memo_key(inputs: &[RuntimeValue], resource_revision: &str) -> OperationMemoKey {
    let input_fingerprints = inputs
        .iter()
        .map(|input| match input.clone() {
            RuntimeValue::Scalar(value) => {
                ValueFingerprint::from_stored_value(&StoredValue::scalar(value))
            }
            RuntimeValue::Artifact(artifact) => {
                ValueFingerprint::from_stored_value(&artifact.into_stored_value())
            }
            RuntimeValue::Stream(_) => panic!("materialized inputs are cacheable"),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    OperationMemoKey {
        operation: OperationStableId::new("events/test::memoized-operation").unwrap(),
        input_fingerprints,
        resource_versions: BTreeMap::from([(
            ResourceKey::new("variables/relevant"),
            ResourceVersion::new(resource_revision),
        )]),
        semantics_version: ExecutionSemanticsVersion::from_bytes([7; 32]),
        computation_settings: ComputationSettingsFingerprint::new(
            EffectiveComputationSettings::default(),
        ),
        demand: DemandFingerprint::from_bytes([9; 32]),
    }
}

#[test]
fn per_run_memoization_demand_fingerprints_are_frame_specific_without_sentinels() {
    let mut root = plan(
        vec![operation("demand", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    root.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut root);

    assert_ne!(
        DemandFingerprint::for_root(&root, None),
        DemandFingerprint::for_root(&root, Some([0; 32]))
    );
    let mut different_publication = root.clone();
    different_publication.results[0].name = "other".into();
    different_publication.publications = Box::new([PlannedPublication::GraphResult {
        name: "other".into(),
        output: different_publication.results[0].output.clone(),
        value: ValueRef::new(0),
    }]);
    assert_ne!(
        DemandFingerprint::for_root(&root, None),
        DemandFingerprint::for_root(&different_publication, None)
    );

    let target = id("functions/callee", FunctionPlanHandle::new);
    let first_arguments = Box::new([CallArgumentBinding {
        caller_source: ValueRef::new(0),
        callee_destination: ValueRef::new(1),
    }]);
    let second_arguments = Box::new([CallArgumentBinding {
        caller_source: ValueRef::new(0),
        callee_destination: ValueRef::new(2),
    }]);
    let results = Box::new([CallResultBinding {
        callee_source: ValueRef::new(3),
        caller_destination: ValueRef::new(4),
        production: Some(OutputProduction::FullyMaterialized),
    }]);
    assert_ne!(
        DemandFingerprint::for_callee(&root, &target, &first_arguments[..], &results[..]),
        DemandFingerprint::for_callee(&root, &target, &second_arguments[..], &results[..])
    );
}

#[test]
fn per_run_memoization_same_key_produces_once() {
    let memo = SessionMemoization::new();
    let key = per_run_memo_key(&[Value::Integer(7).into()], "1");
    let calls = AtomicUsize::new(0);

    for _ in 0..2 {
        let outputs = memo
            .get_or_produce(key.clone(), &CancellationToken::new(), || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(vec![ResultId::new(8)].into_boxed_slice())
            })
            .unwrap();
        assert_eq!(outputs.as_ref(), &[ResultId::new(8)]);
    }

    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn per_run_memoization_different_typed_inputs_produce_separately() {
    let memo = SessionMemoization::new();
    let calls = AtomicUsize::new(0);

    for input in [
        RuntimeValue::from(Value::Integer(7)),
        RuntimeValue::from(Value::String("7".into())),
        RuntimeValue::Artifact(Artifact::new(ArtifactKind::Buffered, [Value::Integer(7)])),
        RuntimeValue::Artifact(Artifact::new(ArtifactKind::Buffered, [Value::Integer(7)])),
        RuntimeValue::Artifact(Artifact::new(ArtifactKind::Collected, [Value::Integer(7)])),
    ] {
        let key = per_run_memo_key(&[input], "1");
        memo.get_or_produce(key, &CancellationToken::new(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new([]))
        })
        .unwrap();
    }

    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[test]
fn per_run_memoization_relevant_resource_revision_is_part_of_the_key() {
    let memo = SessionMemoization::new();
    let calls = AtomicUsize::new(0);

    for revision in ["41", "42"] {
        memo.get_or_produce(
            per_run_memo_key(&[Value::Null.into()], revision),
            &CancellationToken::new(),
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new([]))
            },
        )
        .unwrap();
    }

    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn per_run_memoization_uses_only_operation_resource_versions() {
    let mut memoized = operation("memo_resource", &[], &[]);
    memoized.cache_policy = CachePolicy::PerRun;
    memoized.resource_dependencies = Box::new([ResourceKey::new("variables/relevant")]);
    let mut execution_plan = plan(
        vec![memoized],
        0,
        StructuredControlRegion::Sequence(Box::new([])),
    );
    execution_plan.provenance.basis.resource_versions = BTreeMap::from([
        (
            ResourceKey::new("variables/relevant"),
            ResourceVersion::new("1"),
        ),
        (
            ResourceKey::new("variables/unrelated"),
            ResourceVersion::new("1"),
        ),
    ]);
    let memo = SessionMemoization::new();
    let calls = AtomicUsize::new(0);

    for (unrelated, relevant) in [("1", "1"), ("2", "1"), ("2", "2")] {
        execution_plan.provenance.basis.resource_versions.insert(
            ResourceKey::new("variables/unrelated"),
            ResourceVersion::new(unrelated),
        );
        execution_plan.provenance.basis.resource_versions.insert(
            ResourceKey::new("variables/relevant"),
            ResourceVersion::new(relevant),
        );
        let versions = super::super::scheduler::operation_resource_versions(
            &execution_plan,
            OperationIndex::new(0),
        )
        .expect("declared relevant version exists");
        let key = OperationMemoKey::from_inputs(
            execution_plan.operations[0].stable_id.clone(),
            &[],
            &ResultStore::new(),
            versions,
            execution_plan.operations[0].semantics_version,
            EffectiveComputationSettings::default(),
            DemandFingerprint::from_bytes([9; 32]),
        )
        .unwrap();
        memo.get_or_produce(key, &CancellationToken::new(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new([]))
        })
        .unwrap();
    }

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    execution_plan
        .provenance
        .basis
        .resource_versions
        .remove(&ResourceKey::new("variables/relevant"));
    assert!(
        super::super::scheduler::operation_resource_versions(
            &execution_plan,
            OperationIndex::new(0)
        )
        .is_none()
    );
}

#[test]
fn per_run_memoization_concurrent_same_key_has_one_producer_and_waiter_cancel_isolated() {
    let memo = Arc::new(SessionMemoization::new());
    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let producer_started = Arc::new(Barrier::new(2));
    let release_producer = Arc::new(Barrier::new(2));
    let calls = Arc::new(AtomicUsize::new(0));

    let producer = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let producer_started = Arc::clone(&producer_started);
        let release_producer = Arc::clone(&release_producer);
        let calls = Arc::clone(&calls);
        thread::spawn(move || {
            memo.get_or_produce(key, &CancellationToken::new(), || {
                calls.fetch_add(1, Ordering::SeqCst);
                producer_started.wait();
                release_producer.wait();
                Ok(vec![ResultId::new(2)].into_boxed_slice())
            })
        })
    };
    producer_started.wait();

    let cancelled = CancellationToken::new();
    let cancelled_waiter = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let cancelled = cancelled.clone();
        thread::spawn(move || memo.get_or_produce(key, &cancelled, || panic!("waiter produced")))
    };
    cancelled.cancel();

    let successful_waiter = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        thread::spawn(move || {
            memo.get_or_produce(key, &CancellationToken::new(), || panic!("waiter produced"))
        })
    };
    release_producer.wait();

    assert_eq!(cancelled_waiter.join().unwrap(), Err(RunError::Cancelled));
    assert_eq!(
        producer.join().unwrap().unwrap().as_ref(),
        &[ResultId::new(2)]
    );
    assert_eq!(
        successful_waiter.join().unwrap().unwrap().as_ref(),
        &[ResultId::new(2)]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn per_run_memoization_producer_panic_removes_flight_and_wakes_waiter() {
    let memo = Arc::new(SessionMemoization::new());
    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let producer_started = Arc::new(Barrier::new(2));
    let waiter_registered = Arc::new(Barrier::new(2));
    let release_producer = Arc::new(Barrier::new(2));

    let producer = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let producer_started = Arc::clone(&producer_started);
        let release_producer = Arc::clone(&release_producer);
        thread::spawn(move || {
            memo.get_or_produce(key, &CancellationToken::new(), || {
                producer_started.wait();
                release_producer.wait();
                panic!("producer panic sentinel")
            })
        })
    };
    producer_started.wait();

    let waiter = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let waiter_registered = Arc::clone(&waiter_registered);
        thread::spawn(move || {
            memo.get_or_produce_with_commit_checkpoint(
                key,
                &CancellationToken::new(),
                || panic!("waiter produced"),
                |checkpoint| {
                    if checkpoint == MemoCommitCheckpoint::WaiterRegistered {
                        waiter_registered.wait();
                    }
                },
            )
        })
    };
    waiter_registered.wait();
    release_producer.wait();

    let expected = Err(RunError::InvalidPlan(
        "memoization producer panicked".into(),
    ));
    assert!(producer.join().is_err(), "producer panic must unwind");
    assert_eq!(waiter.join().unwrap(), expected);

    assert_eq!(
        memo.get_or_produce(key, &CancellationToken::new(), || Ok(Box::new([])))
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn per_run_memoization_producer_error_is_removed() {
    let memo = SessionMemoization::new();
    let key = per_run_memo_key(&[Value::Null.into()], "1");
    let calls = AtomicUsize::new(0);

    let first = memo.get_or_produce(key.clone(), &CancellationToken::new(), || {
        calls.fetch_add(1, Ordering::SeqCst);
        Err(RunError::InvalidPlan("failed".into()))
    });
    let second = memo.get_or_produce(key, &CancellationToken::new(), || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new([]))
    });

    assert_eq!(first, Err(RunError::InvalidPlan("failed".into())));
    assert_eq!(second.unwrap().len(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn per_run_memoization_producer_cancellation_is_not_cached() {
    let memo = SessionMemoization::new();
    let key = per_run_memo_key(&[Value::Null.into()], "1");
    let calls = AtomicUsize::new(0);

    let cancelled = CancellationToken::new();
    let producer_token = cancelled.clone();
    let first = memo.get_or_produce(key.clone(), &cancelled, || {
        calls.fetch_add(1, Ordering::SeqCst);
        producer_token.cancel();
        Ok(Box::new([]))
    });
    let second = memo.get_or_produce(key, &CancellationToken::new(), || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new([]))
    });

    assert_eq!(first, Err(RunError::Cancelled));
    assert_eq!(second.unwrap().len(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn per_run_memoization_cancellation_before_commit_does_not_cache() {
    let memo = Arc::new(SessionMemoization::new());
    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let cancellation = CancellationToken::new();
    let at_commit = Arc::new(Barrier::new(2));
    let release_commit = Arc::new(Barrier::new(2));
    let calls = Arc::new(AtomicUsize::new(0));

    let producer = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let cancellation = cancellation.clone();
        let at_commit = Arc::clone(&at_commit);
        let release_commit = Arc::clone(&release_commit);
        let calls = Arc::clone(&calls);
        thread::spawn(move || {
            memo.get_or_produce_with_commit_checkpoint(
                key,
                &cancellation,
                || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(Box::new([]))
                },
                |checkpoint| {
                    if checkpoint == MemoCommitCheckpoint::BeforeCommit {
                        at_commit.wait();
                        release_commit.wait();
                    }
                },
            )
        })
    };
    at_commit.wait();
    cancellation.cancel();
    release_commit.wait();

    assert_eq!(producer.join().unwrap(), Err(RunError::Cancelled));
    memo.get_or_produce(key, &CancellationToken::new(), || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new([]))
    })
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn per_run_memoization_cancellation_after_commit_keeps_cache() {
    let memo = Arc::new(SessionMemoization::new());
    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let cancellation = CancellationToken::new();
    let committed = Arc::new(Barrier::new(2));
    let release_producer = Arc::new(Barrier::new(2));
    let calls = Arc::new(AtomicUsize::new(0));

    let producer = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let cancellation = cancellation.clone();
        let committed = Arc::clone(&committed);
        let release_producer = Arc::clone(&release_producer);
        let calls = Arc::clone(&calls);
        thread::spawn(move || {
            memo.get_or_produce_with_commit_checkpoint(
                key,
                &cancellation,
                || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(Box::new([]))
                },
                |checkpoint| {
                    if checkpoint == MemoCommitCheckpoint::Committed {
                        committed.wait();
                        release_producer.wait();
                    }
                },
            )
        })
    };
    committed.wait();
    cancellation.cancel();
    release_producer.wait();

    assert_eq!(producer.join().unwrap().unwrap().len(), 0);
    memo.get_or_produce(key, &CancellationToken::new(), || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new([]))
    })
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn session_memoization_rejects_unknown_input_result() {
    assert!(
        OperationMemoKey::from_inputs(
            OperationStableId::new("events/test::missing-result").unwrap(),
            &[ResultId::new(u64::MAX)],
            &ResultStore::new(),
            BTreeMap::new(),
            ExecutionSemanticsVersion::from_bytes([7; 32]),
            EffectiveComputationSettings::default(),
            DemandFingerprint::from_bytes([9; 32]),
        )
        .is_none()
    );
}

#[test]
fn per_run_memoization_run_finalization_releases_entries() {
    let memo = SessionMemoization::new();
    let key = per_run_memo_key(&[Value::Null.into()], "1");
    let calls = AtomicUsize::new(0);
    memo.get_or_produce(key.clone(), &CancellationToken::new(), || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new([]))
    })
    .unwrap();
    memo.finalize();
    assert_eq!(
        memo.get_or_produce(key, &CancellationToken::new(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new([]))
        }),
        Err(RunError::Cancelled)
    );

    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn per_run_memoization_finalize_wakes_waiter_and_prevents_late_commit() {
    let memo = Arc::new(SessionMemoization::new());
    let key = per_run_memo_key(&[Value::Integer(1).into()], "1");
    let producer_started = Arc::new(Barrier::new(2));
    let waiter_registered = Arc::new(Barrier::new(2));
    let release_producer = Arc::new(Barrier::new(2));

    let producer = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let producer_started = Arc::clone(&producer_started);
        let release_producer = Arc::clone(&release_producer);
        thread::spawn(move || {
            memo.get_or_produce(key, &CancellationToken::new(), || {
                producer_started.wait();
                release_producer.wait();
                Ok(Box::new([]))
            })
        })
    };
    producer_started.wait();

    let (settled_tx, settled_rx) = mpsc::channel();
    let waiter = {
        let memo = Arc::clone(&memo);
        let key = key.clone();
        let waiter_registered = Arc::clone(&waiter_registered);
        thread::spawn(move || {
            settled_tx
                .send(memo.get_or_produce_with_commit_checkpoint(
                    key,
                    &CancellationToken::new(),
                    || panic!("waiter produced"),
                    |checkpoint| {
                        if checkpoint == MemoCommitCheckpoint::WaiterRegistered {
                            waiter_registered.wait();
                        }
                    },
                ))
                .unwrap();
        })
    };
    waiter_registered.wait();
    memo.finalize();

    assert_eq!(
        settled_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        Err(RunError::Cancelled)
    );
    release_producer.wait();
    assert_eq!(producer.join().unwrap(), Err(RunError::Cancelled));
    waiter.join().unwrap();

    let late_calls = AtomicUsize::new(0);
    let late = memo.get_or_produce(key, &CancellationToken::new(), || {
        late_calls.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new([]))
    });
    assert_eq!(late, Err(RunError::Cancelled));
    assert_eq!(late_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn per_run_memoization_finalize_owner_lock_rejects_late_lookup() {
    let memo = Arc::new(SessionMemoization::new());
    let terminal_set = Arc::new(Barrier::new(2));
    let release_finalize = Arc::new(Barrier::new(2));
    let finalizer = {
        let memo = Arc::clone(&memo);
        let terminal_set = Arc::clone(&terminal_set);
        let release_finalize = Arc::clone(&release_finalize);
        thread::spawn(move || {
            memo.finalize_with_checkpoint(|| {
                terminal_set.wait();
                release_finalize.wait();
            });
        })
    };
    terminal_set.wait();

    let calls = Arc::new(AtomicUsize::new(0));
    let late = {
        let memo = Arc::clone(&memo);
        let calls = Arc::clone(&calls);
        thread::spawn(move || {
            memo.get_or_produce(
                per_run_memo_key(&[Value::Null.into()], "1"),
                &CancellationToken::new(),
                || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    Ok(Box::new([]))
                },
            )
        })
    };
    release_finalize.wait();

    finalizer.join().unwrap();
    assert_eq!(late.join().unwrap(), Err(RunError::Cancelled));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn per_run_memoization_new_run_is_isolated() {
    let key = per_run_memo_key(&[Value::Null.into()], "1");
    let calls = AtomicUsize::new(0);

    for _ in 0..2 {
        let memo = SessionMemoization::new();
        memo.get_or_produce(key.clone(), &CancellationToken::new(), || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new([]))
        })
        .unwrap();
    }

    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn cache_policies_have_exact_two_run_semantics() {
    for (policy, expected_calls) in [
        (CachePolicy::Disabled, 2),
        (CachePolicy::PerRun, 2),
        (CachePolicy::PerSession, 1),
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        let mut kernels = KernelRegistry::new();
        kernels
            .register(
                id("policy_matrix", KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    observed.fetch_add(1, Ordering::SeqCst);
                    Ok(vec![Value::Integer(7).into()])
                }),
            )
            .unwrap();
        let mut cached = operation("policy_matrix", &[], &[0]);
        cached.cache_policy = policy;
        let mut execution_plan = plan(
            vec![cached],
            1,
            StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(
                OperationIndex::new(0),
            )])),
        );
        execution_plan.results = Box::new([PlanResult {
            name: "result".into(),
            output: stable_output("policy_result"),
            value: ValueRef::new(0),
        }]);
        publish_graph_results(&mut execution_plan);
        let resources = no_resources();
        let executor = RunExecutor::new(
            &kernels,
            &resources,
            &NoFunctions,
            ResultStore::new(),
            Arc::new(SessionMemoization::new()),
        );

        executor
            .run(&execution_plan, CancellationToken::new())
            .unwrap();
        executor
            .run(&execution_plan, CancellationToken::new())
            .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), expected_calls, "{policy:?}");
    }
}

#[test]
fn computation_settings_change_misses_session_cache() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("settings_cache", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Value::Integer(7).into()])
            }),
        )
        .unwrap();
    let mut cached = operation("settings_cache", &[], &[0]);
    cached.cache_policy = CachePolicy::PerSession;
    let mut execution_plan = plan(
        vec![cached],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("settings_result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    let store = ResultStore::new();
    let memo = Arc::new(SessionMemoization::new());
    let resources = no_resources();
    let defaults = crate::project::ProjectComputationSettings::default();
    let mut changed = defaults.clone();
    changed.numeric.tolerance.absolute = 0.25;

    RunExecutor::new(
        &kernels,
        &resources,
        &NoFunctions,
        store.clone(),
        Arc::clone(&memo),
    )
    .with_computation_settings_snapshot(&defaults)
    .run(&execution_plan, CancellationToken::new())
    .unwrap();
    RunExecutor::new(&kernels, &resources, &NoFunctions, store, memo)
        .with_computation_settings_snapshot(&changed)
        .run(&execution_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn invalid_reused_group_entry_is_evicted_and_recomputed_once() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("invalid_reuse", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Value::Integer(7).into()])
            }),
        )
        .unwrap();
    let mut cached = operation("invalid_reuse", &[], &[0]);
    cached.cache_policy = CachePolicy::PerSession;
    let mut first_plan = plan(
        vec![cached],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    first_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("invalid_reuse_result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut first_plan);
    let mut second_plan = first_plan.clone();
    second_plan.operations[0].outputs[0].presentation = ResultPresentation::Plot {
        chart: ResultPlotKind::Line,
    };
    let store = ResultStore::new();
    let memo = Arc::new(SessionMemoization::new());
    let resources = no_resources();
    let executor = RunExecutor::new(&kernels, &resources, &NoFunctions, store, memo);

    executor.run(&first_plan, CancellationToken::new()).unwrap();
    executor
        .run(&second_plan, CancellationToken::new())
        .unwrap();
    executor
        .run(&second_plan, CancellationToken::new())
        .unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn session_memoization_executor_runs_same_key_kernel_once_per_session() {
    let memoized_calls = Arc::new(AtomicUsize::new(0));
    let loop_calls = Arc::new(AtomicUsize::new(0));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("memo_initial", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(0).into()])),
        )
        .unwrap();
    let observed_memoized = Arc::clone(&memoized_calls);
    kernels
        .register(
            id("memo_value", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed_memoized.fetch_add(1, Ordering::SeqCst);
                Ok(vec![Value::Integer(41).into()])
            }),
        )
        .unwrap();
    let observed_loop = Arc::clone(&loop_calls);
    kernels
        .register(
            id("memo_loop", KernelHandle::new),
            FnKernel(move |inputs: &[RuntimeValue]| {
                observed_loop.fetch_add(1, Ordering::SeqCst);
                let RuntimeValue::Scalar(Value::Integer(value)) = &inputs[0] else {
                    return Err(KernelError::new("expected loop integer"));
                };
                let next = value + 1;
                Ok(vec![
                    Value::Integer(next).into(),
                    Value::Bool(next < 3).into(),
                ])
            }),
        )
        .unwrap();

    let mut memoized = operation("memo_value", &[], &[1]);
    memoized.cache_policy = CachePolicy::PerSession;
    let mut execution_plan = plan(
        vec![
            operation("memo_initial", &[], &[0]),
            memoized,
            operation("memo_loop", &[2], &[3, 4]),
        ],
        6,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::Loop {
                body: Box::new(StructuredControlRegion::Sequence(Box::new([
                    ControlStep::Operation(OperationIndex::new(1)),
                    ControlStep::Operation(OperationIndex::new(2)),
                ]))),
                carried: Box::new([LoopCarriedBinding {
                    body_input: ValueRef::new(2),
                    initial_source: ValueRef::new(0),
                    next_source: ValueRef::new(3),
                    result: ValueRef::new(5),
                    production: Some(OutputProduction::FullyMaterialized),
                }]),
                continue_condition: ValueRef::new(4),
                max_iterations: 4,
            })),
        ])),
    );
    execution_plan.value_sources = Box::new([
        PlanValueSource::ControlProduced(ValueRef::new(2), OutputProduction::FullyMaterialized),
        PlanValueSource::ControlProduced(ValueRef::new(5), OutputProduction::FullyMaterialized),
    ]);
    execution_plan.results = Box::new([PlanResult {
        name: "count".into(),
        output: stable_output("count"),
        value: ValueRef::new(5),
    }]);
    publish_graph_results(&mut execution_plan);

    execution_plan.validate().unwrap();
    let resources = no_resources();
    let executor = RunExecutor::new(
        &kernels,
        &resources,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    );
    for expected_run_count in 1..=2 {
        let result = executor
            .run(&execution_plan, CancellationToken::new())
            .unwrap();
        assert_eq!(
            result.value_for_test("count").unwrap(),
            RuntimeValue::from(Value::Integer(3))
        );
        assert_eq!(memoized_calls.load(Ordering::SeqCst), 1);
        assert_eq!(loop_calls.load(Ordering::SeqCst), expected_run_count * 3);
    }
}

#[test]
fn memoization_reuses_ordered_output_result_ids_and_history() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("memo_two_outputs", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![
                    Value::Integer(7).into(),
                    Value::String("second".into()).into(),
                ])
            }),
        )
        .unwrap();
    let first_output = stable_output("z_result");
    let second_output = stable_output("a_report");
    let mut memoized = operation("memo_two_outputs", &[], &[0, 1]);
    memoized.cache_policy = CachePolicy::PerSession;
    memoized.outputs[0].public_output = Some(first_output.clone());
    memoized.outputs[1].public_output = Some(second_output.clone());
    let mut execution_plan = plan(
        vec![memoized],
        2,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([
        PlanResult {
            name: "first".into(),
            output: first_output.clone(),
            value: ValueRef::new(0),
        },
        PlanResult {
            name: "second".into(),
            output: second_output.clone(),
            value: ValueRef::new(1),
        },
    ]);
    publish_graph_results(&mut execution_plan);
    execution_plan.validate().unwrap();

    let store = ResultStore::new();
    let memoization = Arc::new(SessionMemoization::new());
    let resources = no_resources();
    let executor = RunExecutor::new(
        &kernels,
        &resources,
        &NoFunctions,
        store.clone(),
        memoization,
    );
    let first = executor
        .run(&execution_plan, CancellationToken::new())
        .unwrap();
    let second = executor
        .run(&execution_plan, CancellationToken::new())
        .unwrap();
    let first_ids = [first.result_ids["first"], first.result_ids["second"]];
    let second_ids = [second.result_ids["first"], second.result_ids["second"]];

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(first_ids, second_ids);
    assert_ne!(first_ids[0], first_ids[1]);
    for output in [&first_output, &second_output] {
        let history = store.pin_history(output);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].usage, ResultUsage::Produced);
        assert!(matches!(
            history[1].usage,
            ResultUsage::Reused {
                original_activation_id
            } if original_activation_id == history[0].activation_id
        ));
        assert_eq!(history[0].result_id, history[1].result_id);
    }
    assert_eq!(
        store.result(first_ids[0]).unwrap().provenance.run_id,
        first.run_id
    );
}

#[test]
fn memoization_reuses_spill_backed_results_without_copying() {
    let root = materialization_test_root("session-spill-reuse");
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("memo_spill_output", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![RuntimeValue::Artifact(Artifact::new(
                    ArtifactKind::Collected,
                    [Value::String("spill-backed-result".into())],
                ))])
            }),
        )
        .unwrap();
    let mut memoized = operation("memo_spill_output", &[], &[0]);
    memoized.cache_policy = CachePolicy::PerSession;
    let mut execution_plan = plan(
        vec![memoized],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("spill_result"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut execution_plan);
    execution_plan.validate().unwrap();

    let store = ResultStore::new();
    let resources = no_resources();
    let executor = RunExecutor::new(
        &kernels,
        &resources,
        &NoFunctions,
        store.clone(),
        Arc::new(SessionMemoization::new()),
    )
    .with_resource_budgets(materialization_test_budgets(1, 1))
    .with_test_spill_root(root.clone());
    let first = executor
        .run(&execution_plan, CancellationToken::new())
        .unwrap();
    let first_id = first.result_ids["result"];
    let first_result = store.result(first_id).unwrap();
    let ResultState::Ready(first_value) = &first_result.state else {
        panic!("first result must be ready");
    };
    assert!(first_value.is_spill_backed());

    let second = executor
        .run(&execution_plan, CancellationToken::new())
        .unwrap();
    let second_id = second.result_ids["result"];
    let second_result = store.result(second_id).unwrap();
    let ResultState::Ready(second_value) = &second_result.state else {
        panic!("second result must be ready");
    };

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(first_id, second_id);
    assert!(first_value.ptr_eq(second_value));
    drop(first_result);
    drop(second_result);
    drop(first);
    drop(second);
    drop(executor);
    drop(store);
    assert!(std::fs::read_dir(&root).unwrap().next().is_none());
    std::fs::remove_dir(root).unwrap();
}
