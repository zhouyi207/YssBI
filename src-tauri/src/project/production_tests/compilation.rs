use super::*;

#[test]
fn production_compiler_rejects_wrong_scope_and_duplicate_shell_nodes() {
    let project = temp_project_with_empty_graph("compiler-shell-diagnostics");
    let state = project.state();
    let first = node("yssbi.project.function.entry");
    let second = node("yssbi.project.function.entry");
    let patch = GraphDocumentPatch::new(vec![
        GraphDocumentOperation::InsertNode { node: first },
        GraphDocumentOperation::InsertNode { node: second },
    ]);
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                ResourceRevision::from_graph_revision(GraphRevision::INITIAL),
                OperationId::new(),
                patch,
            ),
        )
        .unwrap();

    let error = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap_err();
    assert!(error.contains("compiler.node.scope_mismatch"));
    assert!(error.contains("compiler.node.managed_singleton"));
}

#[test]
fn unrelated_resource_mutation_preserves_published_compilation() {
    let used = test_variable("Used");
    let unrelated = test_variable("Unrelated");
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    let mut variable_node = node("yssbi.project.variable.get");
    variable_node.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", used.id)),
    );
    graph.document.nodes.insert(variable_node.id, variable_node);
    let mut data = ProjectData::new();
    data.variables.insert(used.id, used.clone());
    data.variables.insert(unrelated.id, unrelated.clone());
    data.graphs.insert(graph_path(), graph);
    let project = crate::project::fixtures::TempProject::activate(
        "exact-resource-publication-freshness",
        data,
    );
    let state = project.state();

    state.graph_projection(&graph_path(), "en-US").unwrap();
    let original = state
        .published_compile_ids_for_test(&graph_path())
        .unwrap()
        .0;

    state
        .update_variable(
            &unrelated.id,
            Some("Unrelated changed".into()),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let after_unrelated = state
        .published_compile_ids_for_test(&graph_path())
        .unwrap()
        .0;
    assert_eq!(after_unrelated, original);

    state
        .update_variable(
            &used.id,
            Some("Used changed".into()),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let after_dependency = state
        .published_compile_ids_for_test(&graph_path())
        .unwrap()
        .0;
    assert_ne!(after_dependency, original);

    for dependency in [
        "functions/Used.yssbi-function",
        "variables/00000000-0000-0000-0000-000000000701",
        "databases/used",
    ] {
        let coordinator = crate::node_system::compiler::CompileCoordinator::<&str, &str>::new();
        let request_basis = crate::node_system::compiler::compilation_basis(
            GraphRevision::INITIAL,
            crate::node_system::registry::RegistryFingerprint::from_bytes([7; 32]),
            Default::default(),
        );
        let task = match coordinator.request(document_path(), request_basis.clone()) {
            crate::node_system::compiler::ScheduleOutcome::Start(task) => task,
            crate::node_system::compiler::ScheduleOutcome::Coalesced { .. }
            | crate::node_system::compiler::ScheduleOutcome::Exhausted => unreachable!(),
        };
        let dependency_key = crate::node_system::analysis::ResourceKey::new(dependency);
        let unrelated_key = crate::node_system::analysis::ResourceKey::new("variables/unrelated");
        let version_one = crate::node_system::analysis::ResourceVersion::new("1");
        let mut current_versions = std::collections::BTreeMap::from([
            (dependency_key.clone(), version_one.clone()),
            (
                unrelated_key.clone(),
                crate::node_system::analysis::ResourceVersion::new("1"),
            ),
        ]);
        let final_basis = crate::node_system::compiler::compilation_basis(
            GraphRevision::INITIAL,
            request_basis.registry_fingerprint.clone(),
            std::collections::BTreeMap::from([(dependency_key.clone(), version_one)]),
        );
        coordinator.publish_tracked(
            &task,
            &request_basis,
            &current_versions,
            &final_basis,
            crate::node_system::compiler::CompileProducts {
                analysis: "analysis",
                has_blocking_diagnostics: false,
                plan: Some("plan"),
            },
        );
        coordinator.finish(&document_path(), task.compile_id);

        current_versions.insert(
            unrelated_key,
            crate::node_system::analysis::ResourceVersion::new("2"),
        );
        let reused = coordinator
            .get_current_tracked(&document_path(), &request_basis, &current_versions)
            .unwrap();
        assert_eq!(reused.0.compile_id, task.compile_id);

        current_versions.insert(
            dependency_key,
            crate::node_system::analysis::ResourceVersion::new("2"),
        );
        assert!(
            coordinator
                .get_current_tracked(&document_path(), &request_basis, &current_versions)
                .is_none()
        );
        let replacement = match coordinator.request(document_path(), request_basis.clone()) {
            crate::node_system::compiler::ScheduleOutcome::Start(task) => task,
            crate::node_system::compiler::ScheduleOutcome::Coalesced { .. }
            | crate::node_system::compiler::ScheduleOutcome::Exhausted => unreachable!(),
        };
        assert_ne!(replacement.compile_id, task.compile_id);
    }
}

#[test]
fn cached_projection_load_preserves_authority_and_compile_product() {
    let (state, root) = active_state_with_valid_constant_graph("cached-projection-load-reuse");
    let instance = state.capture_project_session().unwrap().instance_id;
    let before_compile = crate::node_system::compiler::compile_snapshot_invocations();

    let expected = state.graph_projection(&graph_path(), "en-US").unwrap();
    let compile_ids = state.published_compile_ids_for_test(&graph_path()).unwrap();
    let generation = state.authority_generation_for_test();

    let first = state
        .load_graph_projection(&instance, &graph_path(), 1, "en-US")
        .unwrap();
    let second = state
        .load_graph_projection(&instance, &graph_path(), 2, "en-US")
        .unwrap();

    assert_eq!(first, expected);
    assert_eq!(second, expected);
    assert_eq!(state.authority_generation_for_test(), generation);
    assert_eq!(
        state.published_compile_ids_for_test(&graph_path()),
        Some(compile_ids),
    );
    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before_compile,
        1,
        "cached projection loads must reuse the existing compile product",
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn projection_and_execution_reuse_one_compile_product() {
    let (state, root) = active_state_with_valid_constant_graph("projection-execution-reuse");
    let before = crate::node_system::compiler::compile_snapshot_invocations();

    state.graph_projection(&graph_path(), "en-US").unwrap();
    state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();

    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        1
    );
    let (analysis_id, plan_id) = state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_eq!(plan_id, Some(analysis_id));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn default_requested_and_preview_demands_reuse_one_compilation_basis() {
    let (state, root) = active_state_with_valid_constant_graph("demand-variant-reuse");
    let first_node = state.get_data().unwrap().graphs[&graph_path()]
        .document
        .nodes
        .keys()
        .next()
        .copied()
        .unwrap();
    let mut second = node("yssbi.constant.int64");
    second.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("value").unwrap(),
        serde_json::json!(9),
    );
    let second_node = second.id;
    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode { node: second }]),
            ),
        )
        .unwrap();
    let output = |node_id| crate::node_system::plan::GraphOutputRef {
        graph_path: document_path(),
        port: crate::graph_document::PortAddress::declared(
            node_id,
            crate::node_system::protocol::PortKey::new("value").unwrap(),
        ),
    };
    let demand = |output| crate::node_system::plan::ExecutionDemand::Outputs {
        outputs: Box::new([output]),
        include_default_results: false,
    };
    let before = crate::node_system::compiler::compile_snapshot_invocations();

    let default_run = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let first_output = output(first_node);
    let first_run = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &demand(first_output.clone()),
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let second_run = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &demand(output(second_node)),
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let preview_events = DemandRunEvents::default();
    let preview_run = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::PinPreview {
                output: first_output.clone(),
                generation: 17,
            },
            &preview_events,
        )
        .unwrap();
    state.graph_projection(&graph_path(), "en-US").unwrap();

    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        1
    );
    assert_eq!(
        default_run.provenance.compile_id,
        first_run.provenance.compile_id
    );
    assert_eq!(
        first_run.provenance.compile_id,
        second_run.provenance.compile_id
    );
    assert_eq!(
        first_run.provenance.compile_id,
        preview_run.provenance.compile_id
    );
    assert_eq!(first_run.result_ids.len(), 1);
    assert_eq!(second_run.result_ids.len(), 1);
    assert_eq!(preview_run.result_ids.len(), 1);
    let preview_result_name = format!("requested.{}", first_output.port);
    let result_id = preview_run.result_ids[preview_result_name.as_str()];
    let stored = state
        .result(result_id)
        .unwrap()
        .expect("preview result is stored");
    assert_eq!(stored.provenance.run_id, preview_run.run_id);
    let preview_events = preview_events.0.lock().unwrap();
    assert_eq!(
        preview_events
            .iter()
            .filter(|event| matches!(
                &event.kind,
                crate::node_system::runtime::RunEventKind::PinPreviewResultReady {
                    output,
                    generation: 17,
                    ..
                } if output == &first_output
            ))
            .count(),
        1,
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn graph_basis_replacement_discards_old_demand_variants() {
    let (state, root) = active_state_with_valid_constant_graph("demand-variant-invalidation");
    let node_id = state.get_data().unwrap().graphs[&graph_path()]
        .document
        .nodes
        .keys()
        .next()
        .copied()
        .unwrap();
    let demand = crate::node_system::plan::ExecutionDemand::Outputs {
        outputs: Box::new([crate::node_system::plan::GraphOutputRef {
            graph_path: document_path(),
            port: crate::graph_document::PortAddress::declared(
                node_id,
                crate::node_system::protocol::PortKey::new("value").unwrap(),
            ),
        }]),
        include_default_results: false,
    };
    state
        .execute_graph_for_current_project_for_test(&graph_path(), &demand, &NOOP_RUN_EVENT_SINK)
        .unwrap();
    let (old_compile_id, old_variants) = state
        .published_variant_cache_state_for_test(&graph_path())
        .unwrap();
    assert_eq!(old_variants, 1);

    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node("yssbi.constant.int64"),
                }]),
            ),
        )
        .unwrap();
    state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let (new_compile_id, new_variants) = state
        .published_variant_cache_state_for_test(&graph_path())
        .unwrap();

    assert_ne!(old_compile_id, new_compile_id);
    assert_eq!(new_variants, 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unrelated_authority_generation_change_preserves_execution_authority() {
    let (state, root) = active_state_with_valid_constant_graph("variant-authority-mismatch");
    let observed = std::sync::Arc::new(std::sync::Mutex::new(None));
    let observed_for_hook = std::sync::Arc::clone(&observed);
    let authority_state = state.clone();
    state.set_execution_before_final_gate_test_hook(std::sync::Arc::new(move || {
        *observed_for_hook.lock().unwrap() =
            authority_state.published_variant_cache_state_for_test(&graph_path());
        authority_state
            .mutation_publication
            .lock()
            .unwrap()
            .advance_authority_generation();
    }));
    let events = DemandRunEvents::default();

    state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &events,
        )
        .unwrap();
    let (compile_id, variants) = observed.lock().unwrap().unwrap();
    assert_eq!(variants, 1);
    assert_eq!(
        state.published_compile_ids_for_test(&graph_path()),
        Some((compile_id, Some(compile_id))),
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn blocking_recompile_clears_published_execution_plan() {
    let (state, root) = active_state_with_valid_constant_graph("blocking-recompile");
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let (valid_compile_id, valid_plan_id) =
        state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_eq!(valid_plan_id, Some(valid_compile_id));
    let coordinator = state.compile_coordinator.read().unwrap().clone();

    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node("yssbi.test.missing"),
                }]),
            ),
        )
        .unwrap();

    assert!(!coordinator.contains_slot_for_test(&document_path()));
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let (blocking_compile_id, blocking_plan_id) =
        state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_ne!(blocking_compile_id, valid_compile_id);
    assert_eq!(blocking_plan_id, None);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn newer_graph_basis_replaces_older_published_plan() {
    let (state, root) = active_state_with_valid_constant_graph("stale-compile-plan");
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let (first_compile_id, first_plan_id) =
        state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_eq!(first_plan_id, Some(first_compile_id));

    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node("yssbi.constant.int64"),
                }]),
            ),
        )
        .unwrap();

    let (gate_paused_tx, gate_paused_rx) = std::sync::mpsc::channel();
    let (release_gate_tx, release_gate_rx) = std::sync::mpsc::channel();
    let release_gate_rx = std::sync::Mutex::new(release_gate_rx);
    let first_gate = std::sync::atomic::AtomicBool::new(true);
    state.set_compile_before_authority_gate_test_hook(std::sync::Arc::new(move || {
        if first_gate.swap(false, std::sync::atomic::Ordering::AcqRel) {
            gate_paused_tx.send(()).unwrap();
            release_gate_rx.lock().unwrap().recv().unwrap();
        }
    }));
    let stale_state = state.clone();
    let stale = std::thread::spawn(move || stale_state.graph_projection(&graph_path(), "en-US"));
    gate_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    release_gate_tx.send(()).unwrap();
    stale.join().unwrap().unwrap();

    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                ResourceRevision::from_graph_revision(GraphRevision::new(2)),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node("yssbi.constant.int64"),
                }]),
            ),
        )
        .unwrap();

    state.graph_projection(&graph_path(), "en-US").unwrap();
    let (current_compile_id, current_plan_id) =
        state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_ne!(current_compile_id, first_compile_id);
    assert_eq!(current_plan_id, Some(current_compile_id));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn graph_unload_invalidates_compile_slot() {
    let (state, root) = active_state_with_valid_constant_graph("unload-compile-slot");
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let coordinator = state.compile_coordinator.read().unwrap().clone();
    assert!(coordinator.contains_slot_for_test(&document_path()));

    state.unload_graph_resource(&graph_path()).unwrap();

    assert!(!coordinator.contains_slot_for_test(&document_path()));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn function_body_mutations_stale_dependent_callers_without_eager_slot_eviction() {
    for entry in ["mutation", "patch"] {
        let (state, function_path, caller_path, _) =
            function_state_with_caller(&format!("FunctionBody{entry}"));
        state.graph_projection(&caller_path, "en-US").unwrap();
        let original_compile_id = state
            .published_compile_ids_for_test(&caller_path)
            .unwrap()
            .0;
        let coordinator = state.compile_coordinator.read().unwrap().clone();
        let caller_document_path = caller_path.clone();
        assert!(coordinator.contains_slot_for_test(&caller_document_path));
        let function_document_path = function_path.clone();

        let request_resource = ResourceKey::Graph(function_document_path);
        if entry == "mutation" {
            state
                .apply_graph_mutation(
                    &function_path,
                    MutationRequest::new(
                        request_resource,
                        ResourceRevision::from_graph_revision(GraphRevision::INITIAL),
                        OperationId::new(),
                        GraphMutation::CreateNode {
                            node: node("yssbi.constant.int64"),
                        },
                    ),
                )
                .unwrap();
        } else {
            state
                .apply_graph_patch(
                    &function_path,
                    MutationRequest::new(
                        request_resource,
                        ResourceRevision::from_graph_revision(GraphRevision::INITIAL),
                        OperationId::new(),
                        GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                            node: node("yssbi.constant.int64"),
                        }]),
                    ),
                )
                .unwrap();
        }

        assert!(
            coordinator.contains_slot_for_test(&caller_document_path),
            "{entry} eagerly evicted a dependent caller slot"
        );
        state.graph_projection(&caller_path, "en-US").unwrap();
        assert_ne!(
            state
                .published_compile_ids_for_test(&caller_path)
                .unwrap()
                .0,
            original_compile_id,
            "{entry} reused a caller whose exact function dependency changed",
        );
    }
}

#[test]
fn project_replacement_detaches_old_compile_generation_and_populated_variants() {
    let (state, root) = active_state_with_valid_constant_graph("replace-compile-generation");
    state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap();
    let (detached_compile_id, variants) = state
        .published_variant_cache_state_for_test(&graph_path())
        .unwrap();
    assert_eq!(variants, 1);
    let detached = state.compile_coordinator.read().unwrap().clone();
    assert!(detached.contains_slot_for_test(&document_path()));

    state.activate_project_fixture("replacement-project".into(), ProjectData::new());

    let current = state.compile_coordinator.read().unwrap().clone();
    assert!(!std::sync::Arc::ptr_eq(&detached, &current));
    assert!(!detached.contains_slot_for_test(&document_path()));
    assert!(!current.contains_slot_for_test(&document_path()));
    assert!(
        state
            .published_variant_cache_state_for_test(&graph_path())
            .is_none()
    );
    assert!(detached_compile_id.get() > 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn committed_graph_mutation_rejects_environment_from_older_authority_generation() {
    let state = state_with_empty_graph();
    let (capture_paused_tx, capture_paused_rx) = std::sync::mpsc::channel();
    let (release_capture_tx, release_capture_rx) = std::sync::mpsc::channel();
    let release_capture_rx = std::sync::Mutex::new(release_capture_rx);
    let capture_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let capture_count_for_hook = std::sync::Arc::clone(&capture_count);
    state.set_projection_environment_after_path_data_test_hook(std::sync::Arc::new(move || {
        if capture_count_for_hook.fetch_add(1, std::sync::atomic::Ordering::AcqRel) == 0 {
            capture_paused_tx.send(()).unwrap();
            release_capture_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let mutation_state = state.clone();
    let mutation = std::thread::spawn(move || {
        mutation_state.apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                ResourceRevision::from_graph_revision(GraphRevision::INITIAL),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node("yssbi.constant.int64"),
                }]),
            ),
        )
    });
    capture_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    state
        .insert_graph(
            GraphResourcePath::new("events/Unrelated.yssbi-event").unwrap(),
            GraphResourceDocument::new("Unrelated", GraphDocumentKind::Event),
        )
        .unwrap();
    release_capture_tx.send(()).unwrap();

    match mutation.join().unwrap() {
        Ok(_) => {
            assert_eq!(capture_count.load(std::sync::atomic::Ordering::Acquire), 2);
            assert_eq!(
                state.get_data().unwrap().graphs[&graph_path()]
                    .document
                    .revision,
                GraphRevision::new(1)
            );
        }
        Err(MutationConflict::Projection(_)) => {
            assert_eq!(
                state.get_data().unwrap().graphs[&graph_path()]
                    .document
                    .revision,
                GraphRevision::INITIAL
            );
        }
        Err(error) => panic!("unexpected mutation error: {error}"),
    }
}

#[test]
fn graph_projection_retries_when_authority_changes_during_metadata_capture() {
    let (state, root) = active_state_with_valid_constant_graph("graph-projection-capture");
    let capture_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let capture_count_for_hook = std::sync::Arc::clone(&capture_count);
    let (capture_paused_tx, capture_paused_rx) = std::sync::mpsc::channel();
    let (release_capture_tx, release_capture_rx) = std::sync::mpsc::channel();
    let release_capture_rx = std::sync::Mutex::new(release_capture_rx);
    state.set_projection_environment_after_path_data_test_hook(std::sync::Arc::new(move || {
        if capture_count_for_hook.fetch_add(1, std::sync::atomic::Ordering::AcqRel) == 0 {
            capture_paused_tx.send(()).unwrap();
            release_capture_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let projection_state = state.clone();
    let projection =
        std::thread::spawn(move || projection_state.graph_projection(&graph_path(), "en-US"));
    capture_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    {
        let mut publication = state.mutation_publication.lock().unwrap();
        let mut data = state.project_data.write().unwrap();
        data.graphs
            .get_mut(&graph_path())
            .unwrap()
            .document
            .revision = GraphRevision::new(2);
        publication.advance_authority_generation();
    }
    release_capture_tx.send(()).unwrap();

    let projection = projection.join().unwrap().unwrap();
    assert_eq!(projection.source_revision, 2);
    let captures = capture_count.load(std::sync::atomic::Ordering::Acquire);
    assert!(
        captures >= 2,
        "expected invalidated capture to be retried, observed {captures} capture(s)"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn committed_source_keeps_captured_authority_across_unrelated_generation_aba() {
    let (state, function_path, caller_path, resource) =
        function_state_with_caller("CompileSourceAba");
    let authority_state = state.clone();
    state.set_committed_resource_completion_test_hook(std::sync::Arc::new(move || {
        let mut publication = authority_state.mutation_publication.lock().unwrap();
        publication.advance_authority_generation();
        publication.advance_authority_generation();
    }));

    let result = state
        .update_function_signature_observed(
            &current_project_instance_id(&state),
            &function_path,
            "en-US",
            function_signature_request(
                resource,
                GraphRevision::INITIAL,
                Default::default(),
                test_signature(),
            ),
            |_| {},
        )
        .unwrap();

    assert_eq!(
        result.projection_status,
        crate::event::ProjectionStatusDto::Complete {
            expected_graph_paths: vec![
                caller_path.as_str().to_string(),
                function_path.as_str().to_string(),
            ],
        }
    );
    assert_eq!(result.projection_replacements.len(), 2);
}

#[test]
fn function_insert_uses_max_incoming_or_retained_successor_and_reports_overflow() {
    let state = ProjectState::new();
    state.activate_project_fixture("function-insert-revision".into(), ProjectData::new());
    let path = GraphResourcePath::new("functions/Insert.yssbi-function").unwrap();
    state
        .graph_revisions
        .write()
        .unwrap()
        .insert(path.clone(), GraphRevision::new(7));

    let mut low = GraphResourceDocument::new("Insert", GraphDocumentKind::Function);
    low.document.revision = GraphRevision::new(3);
    let inserted = state.insert_graph(path.clone(), low).unwrap();
    assert_eq!(inserted.document.revision, GraphRevision::new(8));
    assert_eq!(
        inserted.function.as_ref().unwrap().revision,
        ResourceRevision::from_graph_revision(inserted.document.revision)
    );
    assert_eq!(
        state.graph_revisions.read().unwrap()[&path],
        GraphRevision::new(8)
    );

    let mut high = GraphResourceDocument::new("Insert", GraphDocumentKind::Function);
    high.document.revision = GraphRevision::new(12);
    let inserted = state.insert_graph(path.clone(), high).unwrap();
    assert_eq!(inserted.document.revision, GraphRevision::new(12));
    assert_eq!(
        state.graph_revisions.read().unwrap()[&path],
        GraphRevision::new(12)
    );

    let overflow = GraphResourcePath::new("functions/Overflow.yssbi-function").unwrap();
    state
        .graph_revisions
        .write()
        .unwrap()
        .insert(overflow.clone(), GraphRevision::new(u64::MAX));
    let before_generation = state.authority_generation_for_test();
    let error = state
        .insert_graph(
            overflow.clone(),
            GraphResourceDocument::new("Overflow", GraphDocumentKind::Function),
        )
        .unwrap_err();
    assert_eq!(error.code(), "resource_revision_overflow");
    assert!(!state.get_data().unwrap().graphs.contains_key(&overflow));
    assert_eq!(
        state.graph_revisions.read().unwrap()[&overflow],
        GraphRevision::new(u64::MAX)
    );
    assert_eq!(state.authority_generation_for_test(), before_generation);
}

#[test]
fn function_patch_remove_and_reinsert_keep_authoritative_revisions_coherent() {
    let (state, root) = state_with_project_path("function-patch-reinsert");
    let path = GraphResourcePath::new("functions/Reinsert.yssbi-function").unwrap();
    let mut original = GraphResourceDocument::new("Reinsert", GraphDocumentKind::Function);
    original.document.revision = GraphRevision::new(4);
    state.insert_graph(path.clone(), original).unwrap();
    let key = ResourceKey::Graph(path.clone());
    let remove_context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: vec![key.clone()],
        expected_revisions: [(key.clone(), ResourceRevision::new(4))]
            .into_iter()
            .collect(),
        expected_absent_resources: Default::default(),
        recovery_marker: Some(state.project_recovery_marker()),
    };

    let removed = state
        .apply_resource_document_patch(
            &remove_context,
            ResourceDocumentPatch::RemoveGraph {
                path: path.clone(),
                revision: ResourceRevision::new(4),
            },
        )
        .unwrap();
    assert_eq!(
        removed.deltas[0].from_revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(4))
    );
    assert_eq!(
        removed.deltas[0].to_revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(5))
    );
    let crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(removed_lifecycle) =
        &removed.deltas[0].payload
    else {
        panic!("expected removal lifecycle delta");
    };
    assert_eq!(
        removed_lifecycle.before.as_ref().unwrap().revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(4))
    );
    assert_eq!(
        state.graph_revisions.read().unwrap()[&path],
        GraphRevision::new(5)
    );
    assert!(!state.get_data().unwrap().graphs.contains_key(&path));

    let insert_context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: Vec::new(),
        expected_revisions: Default::default(),
        expected_absent_resources: [key].into_iter().collect(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    let mut incoming = GraphResourceDocument::new("Reinsert", GraphDocumentKind::Function);
    incoming.document.revision = GraphRevision::new(1);
    let inserted = state
        .apply_resource_document_patch(
            &insert_context,
            ResourceDocumentPatch::InsertGraph {
                path: path.clone(),
                resource: incoming,
            },
        )
        .unwrap();

    let revision = GraphRevision::new(6);
    let data = state.get_data().unwrap();
    assert_eq!(data.graphs[&path].document.revision, revision);
    assert_eq!(
        data.graphs[&path].function.as_ref().unwrap().revision,
        ResourceRevision::from_graph_revision(revision)
    );
    assert_eq!(state.graph_revisions.read().unwrap()[&path], revision);
    assert_eq!(
        inserted.deltas[0].from_revision,
        ResourceRevision::from_graph_revision(GraphRevision::new(5))
    );
    assert_eq!(
        inserted.deltas[0].to_revision,
        ResourceRevision::from_graph_revision(revision)
    );
    let crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(inserted_lifecycle) =
        &inserted.deltas[0].payload
    else {
        panic!("expected insertion lifecycle delta");
    };
    assert_eq!(
        inserted_lifecycle.after.as_ref().unwrap().revision,
        ResourceRevision::from_graph_revision(revision)
    );
    assert_eq!(
        inserted.projection_replacements[0]
            .projection
            .source_revision,
        revision.get()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn function_move_into_tombstone_keeps_document_ledger_delta_and_projection_revision_equal() {
    let (state, root) = state_with_project_path("function-move-target-tombstone");
    let from = GraphResourcePath::new("functions/Before.yssbi-function").unwrap();
    let to = GraphResourcePath::new("functions/After.yssbi-function").unwrap();
    let mut source = GraphResourceDocument::new("Before", GraphDocumentKind::Function);
    source.document.revision = GraphRevision::new(2);
    state.insert_graph(from.clone(), source.clone()).unwrap();
    state
        .graph_revisions
        .write()
        .unwrap()
        .insert(to.clone(), GraphRevision::new(9));
    let source_key = ResourceKey::Graph(from.clone());
    let target_key = ResourceKey::Graph(to.clone());
    let context = ProjectTransactionContext {
        session: state.capture_project_session().unwrap(),
        operation_id: OperationId::new(),
        affected_resources: vec![source_key.clone()],
        expected_revisions: [(source_key, ResourceRevision::new(2))]
            .into_iter()
            .collect(),
        expected_absent_resources: [target_key].into_iter().collect(),
        recovery_marker: Some(state.project_recovery_marker()),
    };
    let mut moved = source.clone();
    moved.name = "After".into();
    moved.document.revision = GraphRevision::new(3);

    let result = state
        .apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::MoveGraph {
                from: from.clone(),
                to: to.clone(),
                moved_before: source,
                moved,
                referenced_graphs_before: Default::default(),
                referenced_graphs: Default::default(),
                loaded_referenced_graphs: Default::default(),
                referenced_variables_before: Default::default(),
                referenced_variables: Default::default(),
            },
        )
        .unwrap();

    let revision = GraphRevision::new(10);
    let data = state.get_data().unwrap();
    let moved = &data.graphs[&to];
    assert_eq!(moved.document.revision, revision);
    assert_eq!(
        moved.function.as_ref().unwrap().revision,
        ResourceRevision::from_graph_revision(revision)
    );
    assert_eq!(state.graph_revisions.read().unwrap()[&to], revision);
    assert_eq!(
        result.deltas[0].to_revision,
        ResourceRevision::from_graph_revision(revision)
    );
    assert_eq!(
        result.projection_replacements[0].projection.source_revision,
        revision.get()
    );
    assert_eq!(
        state.graph_revisions.read().unwrap()[&from],
        GraphRevision::new(3)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn function_load_over_retained_revision_keeps_document_ledger_and_projection_equal() {
    let state = ActivatedProjectState(crate::project::fixtures::TempProject::activate(
        "function-load-retained-revision",
        ProjectData::new(),
    ));
    let path = GraphResourcePath::new("functions/Loaded.yssbi-function").unwrap();
    let mut persisted = GraphResourceDocument::new("Loaded", GraphDocumentKind::Function);
    persisted.document.revision = GraphRevision::new(2);
    state.insert_graph(path.clone(), persisted).unwrap();
    crate::project::fixtures::write_state_graph(&state, &path).unwrap();
    state.unload_graph_resource(&path).unwrap();
    state
        .graph_revisions
        .write()
        .unwrap()
        .insert(path.clone(), GraphRevision::new(8));
    let instance = state.capture_project_session().unwrap().instance_id;

    let projection = state
        .load_graph_projection(&instance, &path, 1, "en-US")
        .unwrap();

    let revision = GraphRevision::new(9);
    let data = state.get_data().unwrap();
    assert_eq!(data.graphs[&path].document.revision, revision);
    assert_eq!(
        data.graphs[&path].function.as_ref().unwrap().revision,
        ResourceRevision::from_graph_revision(revision)
    );
    assert_eq!(state.graph_revisions.read().unwrap()[&path], revision);
    assert_eq!(projection.source_revision, revision.get());

    let reloaded = state
        .load_graph_projection(&instance, &path, 2, "en-US")
        .unwrap();
    let reload_revision = revision;
    let data = state.get_data().unwrap();
    assert_eq!(data.graphs[&path].document.revision, reload_revision);
    assert_eq!(
        data.graphs[&path].function.as_ref().unwrap().revision,
        ResourceRevision::from_graph_revision(reload_revision)
    );
    assert_eq!(
        state.graph_revisions.read().unwrap()[&path],
        reload_revision
    );
    assert_eq!(reloaded.source_revision, reload_revision.get());
}

#[test]
fn project_resource_authority_tracks_missing_tombstones_and_prevents_aba() {
    let state = ProjectState::new();
    state.activate_project_fixture("resource-authority-state".into(), ProjectData::new());
    let function_path = GraphResourcePath::new("functions/Authority.yssbi-function").unwrap();
    let function_key = crate::node_system::analysis::ResourceKey::new(function_path.as_str());
    let variable = test_variable("Authority Variable");
    let variable_id = variable.id;
    let variable_key =
        crate::node_system::analysis::ResourceKey::new(format!("variables/{variable_id}"));
    let database_key = crate::node_system::analysis::ResourceKey::new("databases/authority");
    let keys = || {
        vec![
            function_key.clone(),
            variable_key.clone(),
            database_key.clone(),
        ]
    };

    let missing = state.authoritative_resource_states_for_test(keys());
    assert!(
        missing.values().all(
            |state| state == &crate::node_system::analysis::ResourceObservedState::Absent(None)
        )
    );

    let function = GraphResourceDocument::new("Authority", GraphDocumentKind::Function);
    state
        .insert_graph(function_path.clone(), function.clone())
        .unwrap();
    {
        let mut publication = state.mutation_publication.lock().unwrap();
        let mut data = state.project_data.write().unwrap();
        data.variables.insert(variable_id, variable.clone());
        data.databases.insert(
            "authority".into(),
            crate::database::DatabaseDecl {
                id: "authority".into(),
                engine: crate::database::DatabaseEngine::InMemory {
                    name: "authority".into(),
                },
                schema_version: 1,
                required: false,
                name: "Authority".into(),
            },
        );
        state.variable_revisions.write().unwrap().insert(
            variable_id,
            super::project_state::VariableRevisionEntry::present(ResourceRevision::new(1)),
        );
        state
            .database_authority_revisions
            .write()
            .unwrap()
            .insert("authority".into(), 1);
        publication.advance_authority_generation();
    }
    let present = state.authoritative_resource_states_for_test(keys());
    assert!(present.values().all(|state| matches!(
        state,
        crate::node_system::analysis::ResourceObservedState::Present(_)
    )));

    {
        let mut publication = state.mutation_publication.lock().unwrap();
        let mut data = state.project_data.write().unwrap();
        data.graphs.remove(&function_path);
        data.variables.remove(&variable_id);
        data.databases.remove("authority");
        let mut graph_revisions = state.graph_revisions.write().unwrap();
        let function_tombstone = graph_revisions[&function_path].next();
        graph_revisions.insert(function_path.clone(), function_tombstone);
        let mut variable_revisions = state.variable_revisions.write().unwrap();
        let variable_tombstone = variable_revisions[&variable_id].revision.next();
        variable_revisions.insert(
            variable_id,
            super::project_state::VariableRevisionEntry::deleted(variable_tombstone),
        );
        let mut database_revisions = state.database_authority_revisions.write().unwrap();
        *database_revisions.get_mut("authority").unwrap() += 1;
        publication.advance_authority_generation();
    }
    let tombstones = state.authoritative_resource_states_for_test(keys());
    assert!(tombstones.values().all(|state| matches!(
        state,
        crate::node_system::analysis::ResourceObservedState::Absent(Some(_))
    )));

    state.insert_graph(function_path.clone(), function).unwrap();
    {
        let mut publication = state.mutation_publication.lock().unwrap();
        let mut data = state.project_data.write().unwrap();
        data.variables.insert(variable_id, variable);
        data.databases.insert(
            "authority".into(),
            crate::database::DatabaseDecl {
                id: "authority".into(),
                engine: crate::database::DatabaseEngine::InMemory {
                    name: "authority".into(),
                },
                schema_version: 1,
                required: false,
                name: "Authority".into(),
            },
        );
        let mut variable_revisions = state.variable_revisions.write().unwrap();
        let next_variable = variable_revisions[&variable_id].revision.next();
        variable_revisions.insert(
            variable_id,
            super::project_state::VariableRevisionEntry::present(next_variable),
        );
        *state
            .database_authority_revisions
            .write()
            .unwrap()
            .get_mut("authority")
            .unwrap() += 1;
        publication.advance_authority_generation();
    }
    let recreated = state.authoritative_resource_states_for_test(keys());
    assert!(recreated.values().all(|state| matches!(
        state,
        crate::node_system::analysis::ResourceObservedState::Present(_)
    )));
    assert_ne!(
        present, recreated,
        "same-content recreation must not reuse versions"
    );
}

#[test]
fn captured_source_compiles_once_across_unrelated_mutation() {
    let unrelated = test_variable("Unrelated capture mutation");
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    let mut constant = node("yssbi.constant.int64");
    constant.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("value").unwrap(),
        serde_json::json!(7),
    );
    graph.document.nodes.insert(constant.id, constant);
    let mut data = ProjectData::new();
    data.variables.insert(unrelated.id, unrelated.clone());
    data.graphs.insert(graph_path(), graph);
    let project = crate::project::fixtures::TempProject::activate(
        "compile-captured-source-unrelated-mutation",
        data,
    );
    let state = project.state();
    let hook_state = state.clone();
    let hook_variable = unrelated.id;
    let hook_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_calls_for_hook = std::sync::Arc::clone(&hook_calls);
    state.set_compile_after_source_capture_test_hook(std::sync::Arc::new(move || {
        let attempt = hook_calls_for_hook.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        assert_eq!(
            attempt, 0,
            "stale captured source entered a Start/cancel loop"
        );
        hook_state
            .update_variable(
                &hook_variable,
                Some("Unrelated changed after capture".into()),
                None,
                None,
                None,
                None,
            )
            .unwrap();
    }));
    let before = crate::node_system::compiler::compile_snapshot_invocations();

    state.graph_projection(&graph_path(), "en-US").unwrap();

    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        1,
        "the first captured source must compile and publish exactly once"
    );
    assert_eq!(hook_calls.load(std::sync::atomic::Ordering::Acquire), 1);
}

#[test]
fn captured_dependency_mutation_rejects_publish_and_recompiles_current() {
    let used = test_variable("Captured dependency");
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    let mut variable_node = node("yssbi.project.variable.get");
    variable_node.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", used.id)),
    );
    graph.document.nodes.insert(variable_node.id, variable_node);
    let mut data = ProjectData::new();
    data.variables.insert(used.id, used.clone());
    data.graphs.insert(graph_path(), graph);
    let project = crate::project::fixtures::TempProject::activate(
        "compile-captured-source-dependency-mutation",
        data,
    );
    let state = project.state();
    let hook_state = state.clone();
    let hook_variable = used.id;
    let hook_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_calls_for_hook = std::sync::Arc::clone(&hook_calls);
    state.set_compile_after_source_capture_test_hook(std::sync::Arc::new(move || {
        let attempt = hook_calls_for_hook.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        if attempt == 0 {
            hook_state
                .update_variable(
                    &hook_variable,
                    Some("Dependency changed after capture".into()),
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap();
        } else if attempt > 1 {
            panic!("stale dependency source was compiled more than once");
        }
    }));
    let before = crate::node_system::compiler::compile_snapshot_invocations();

    state.graph_projection(&graph_path(), "en-US").unwrap();

    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        2,
        "the captured compile must finish, fail publication, and recompile current authority"
    );
    assert_eq!(hook_calls.load(std::sync::atomic::Ordering::Acquire), 2);
}

#[test]
fn stale_lifecycle_after_source_capture_returns_without_compiling() {
    let (state, root) = active_state_with_valid_constant_graph("compile-source-stale-lifecycle");
    let mut replacement_data = ProjectData::new();
    replacement_data.graphs.insert(
        graph_path(),
        GraphResourceDocument::new("Replacement", GraphDocumentKind::Event),
    );
    let replacement = crate::project::fixtures::TempProject::activate(
        "compile-source-stale-lifecycle-replacement",
        replacement_data.clone(),
    );
    let replacement_root = replacement.state().capture_project_session().unwrap().root;
    let hook_state = state.clone();
    let hook_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook_calls_for_hook = std::sync::Arc::clone(&hook_calls);
    state.set_compile_after_source_capture_test_hook(std::sync::Arc::new(move || {
        let attempt = hook_calls_for_hook.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        assert_eq!(attempt, 0, "stale lifecycle entered a compile retry loop");
        hook_state.activate_project_fixture(
            replacement_root.as_path().to_string_lossy().into_owned(),
            replacement_data.clone(),
        );
    }));
    let before = crate::node_system::compiler::compile_snapshot_invocations();

    let error = state.graph_projection(&graph_path(), "en-US").unwrap_err();

    assert!(error.contains("stale_project_lifecycle"), "{error}");
    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        0,
        "stale lifecycle must return before real compilation"
    );
    assert_eq!(hook_calls.load(std::sync::atomic::Ordering::Acquire), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn publish_gate_ignores_unrelated_authority_generation_change() {
    let (state, root) = active_state_with_valid_constant_graph("compile-publish-gate");
    let (gate_paused_tx, gate_paused_rx) = std::sync::mpsc::channel();
    let (release_gate_tx, release_gate_rx) = std::sync::mpsc::channel();
    let release_gate_rx = std::sync::Mutex::new(release_gate_rx);
    let first_gate = std::sync::atomic::AtomicBool::new(true);
    state.set_compile_before_authority_gate_test_hook(std::sync::Arc::new(move || {
        if first_gate.swap(false, std::sync::atomic::Ordering::AcqRel) {
            gate_paused_tx.send(()).unwrap();
            release_gate_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let projection_state = state.clone();
    let projection =
        std::thread::spawn(move || projection_state.graph_projection(&graph_path(), "en-US"));
    gate_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    state
        .mutation_publication
        .lock()
        .unwrap()
        .advance_authority_generation();
    release_gate_tx.send(()).unwrap();

    projection.join().unwrap().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn fast_path_gate_revalidates_dependency_mutation_after_candidate_capture() {
    let used = test_variable("Used");
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    let mut variable_node = node("yssbi.project.variable.get");
    variable_node.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", used.id)),
    );
    graph.document.nodes.insert(variable_node.id, variable_node);
    let mut data = ProjectData::new();
    data.variables.insert(used.id, used.clone());
    data.graphs.insert(graph_path(), graph);
    let project = crate::project::fixtures::TempProject::activate(
        "compile-fast-path-exact-dependency-gate",
        data,
    );
    let state = project.state();
    let before = crate::node_system::compiler::compile_snapshot_invocations();
    state.graph_projection(&graph_path(), "en-US").unwrap();

    let (gate_paused_tx, gate_paused_rx) = std::sync::mpsc::channel();
    let (release_gate_tx, release_gate_rx) = std::sync::mpsc::channel();
    let release_gate_rx = std::sync::Mutex::new(release_gate_rx);
    let first_gate = std::sync::atomic::AtomicBool::new(true);
    state.set_compile_before_authority_gate_test_hook(std::sync::Arc::new(move || {
        if first_gate.swap(false, std::sync::atomic::Ordering::AcqRel) {
            gate_paused_tx.send(()).unwrap();
            release_gate_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let projection_state = state.clone();
    let projection =
        std::thread::spawn(move || projection_state.graph_projection(&graph_path(), "en-US"));
    gate_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    state
        .update_variable(
            &used.id,
            Some("Used changed after candidate capture".into()),
            None,
            None,
            None,
            None,
        )
        .unwrap();
    release_gate_tx.send(()).unwrap();
    projection.join().unwrap().unwrap();

    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        2,
        "the stale fast-path candidate must be rejected and recompiled"
    );
}

#[test]
fn fast_path_gate_ignores_unrelated_authority_generation_change() {
    let (state, root) = active_state_with_valid_constant_graph("compile-fast-path-gate");
    state.graph_projection(&graph_path(), "en-US").unwrap();
    let (gate_paused_tx, gate_paused_rx) = std::sync::mpsc::channel();
    let (release_gate_tx, release_gate_rx) = std::sync::mpsc::channel();
    let release_gate_rx = std::sync::Mutex::new(release_gate_rx);
    let first_gate = std::sync::atomic::AtomicBool::new(true);
    state.set_compile_before_authority_gate_test_hook(std::sync::Arc::new(move || {
        if first_gate.swap(false, std::sync::atomic::Ordering::AcqRel) {
            gate_paused_tx.send(()).unwrap();
            release_gate_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let projection_state = state.clone();
    let projection =
        std::thread::spawn(move || projection_state.graph_projection(&graph_path(), "en-US"));
    gate_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    state
        .mutation_publication
        .lock()
        .unwrap()
        .advance_authority_generation();
    release_gate_tx.send(()).unwrap();

    projection.join().unwrap().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn coalesced_waiters_ignore_unrelated_authority_generation_change() {
    let (state, root) = active_state_with_valid_constant_graph("coalesced-stale-termination");
    let (gate_paused_tx, gate_paused_rx) = std::sync::mpsc::channel();
    let (release_gate_tx, release_gate_rx) = std::sync::mpsc::channel();
    let release_gate_rx = std::sync::Mutex::new(release_gate_rx);
    let first_gate = std::sync::atomic::AtomicBool::new(true);
    state.set_compile_before_authority_gate_test_hook(std::sync::Arc::new(move || {
        if first_gate.swap(false, std::sync::atomic::Ordering::AcqRel) {
            gate_paused_tx.send(()).unwrap();
            release_gate_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let first_state = state.clone();
    let first = std::thread::spawn(move || first_state.graph_projection(&graph_path(), "en-US"));
    gate_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();

    let (waiter_paused_tx, waiter_paused_rx) = std::sync::mpsc::channel();
    let (release_waiter_tx, release_waiter_rx) = std::sync::mpsc::channel();
    let release_waiter_rx = std::sync::Mutex::new(release_waiter_rx);
    state.set_compile_coalesced_before_wait_test_hook(std::sync::Arc::new(move || {
        waiter_paused_tx.send(()).unwrap();
        release_waiter_rx.lock().unwrap().recv().unwrap();
    }));
    let second_state = state.clone();
    let second = std::thread::spawn(move || second_state.graph_projection(&graph_path(), "en-US"));
    waiter_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();

    state
        .mutation_publication
        .lock()
        .unwrap()
        .advance_authority_generation();
    release_waiter_tx.send(()).unwrap();
    release_gate_tx.send(()).unwrap();

    for result in [first.join().unwrap(), second.join().unwrap()] {
        result.unwrap();
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn different_basis_request_compiles_after_authoritative_invalidation() {
    let (state, root) = active_state_with_valid_constant_graph("pending-latest-publication");
    let before = crate::node_system::compiler::compile_snapshot_invocations();
    let (gate_paused_tx, gate_paused_rx) = std::sync::mpsc::channel();
    let (release_gate_tx, release_gate_rx) = std::sync::mpsc::channel();
    let release_gate_rx = std::sync::Mutex::new(release_gate_rx);
    let first_gate = std::sync::atomic::AtomicBool::new(true);
    state.set_compile_before_authority_gate_test_hook(std::sync::Arc::new(move || {
        if first_gate.swap(false, std::sync::atomic::Ordering::AcqRel) {
            gate_paused_tx.send(()).unwrap();
            release_gate_rx.lock().unwrap().recv().unwrap();
        }
    }));

    let active_state = state.clone();
    let active = std::thread::spawn(move || active_state.graph_projection(&graph_path(), "en-US"));
    gate_paused_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    release_gate_tx.send(()).unwrap();
    active.join().unwrap().unwrap();

    state
        .apply_graph_patch(
            &graph_path(),
            MutationRequest::new(
                ResourceKey::Graph(document_path()),
                ResourceRevision::from_graph_revision(GraphRevision::new(1)),
                OperationId::new(),
                GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
                    node: node("yssbi.constant.int64"),
                }]),
            ),
        )
        .unwrap();
    let latest = state.graph_projection(&graph_path(), "en-US").unwrap();
    assert_eq!(latest.source_revision, 2);

    assert_eq!(
        crate::node_system::compiler::compile_snapshot_invocations() - before,
        3
    );
    let (analysis_id, plan_id) = state.published_compile_ids_for_test(&graph_path()).unwrap();
    assert_eq!(plan_id, Some(analysis_id));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn execution_rejects_exact_dependency_change_after_plan_before_run() {
    let used = test_variable("Execution Authority");
    let mut graph = GraphResourceDocument::new("Production", GraphDocumentKind::Event);
    let mut variable_node = node("yssbi.project.variable.get");
    variable_node.parameters.insert(
        crate::node_system::protocol::ParameterKey::new("variable").unwrap(),
        serde_json::json!(format!("variables/{}", used.id)),
    );
    graph.document.nodes.insert(variable_node.id, variable_node);
    let mut data = ProjectData::new();
    data.variables.insert(used.id, used.clone());
    data.graphs.insert(graph_path(), graph);
    let project = crate::project::fixtures::TempProject::activate(
        "execution-exact-dependency-authority",
        data,
    );
    let state = project.state();
    let mutation_state = state.clone();
    state.set_execution_before_final_gate_test_hook(std::sync::Arc::new(move || {
        mutation_state
            .update_variable(
                &used.id,
                Some("Changed before run".into()),
                None,
                None,
                None,
                None,
            )
            .unwrap();
    }));
    let run_entered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let run_entered_for_hook = std::sync::Arc::clone(&run_entered);
    state.set_execution_before_run_test_hook(std::sync::Arc::new(move || {
        run_entered_for_hook.store(true, std::sync::atomic::Ordering::Release);
    }));

    let error = state
        .execute_graph_for_current_project_for_test(
            &graph_path(),
            &crate::node_system::plan::ExecutionDemand::Default,
            &NOOP_RUN_EVENT_SINK,
        )
        .unwrap_err();

    assert_eq!(
        error.kind(),
        crate::project::ProjectExecutionErrorKind::StaleProjectLifecycle,
        "unexpected error: {error}"
    );
    assert!(!run_entered.load(std::sync::atomic::Ordering::Acquire));
}

#[test]
fn function_resource_version_changes_with_name_and_graph_body() {
    let function_path = GraphResourcePath::new("functions/Fingerprint.yssbi-function").unwrap();
    let mut data = ProjectData::new();
    data.graphs.insert(
        function_path.clone(),
        GraphResourceDocument::new("Fingerprint", GraphDocumentKind::Function),
    );
    let key = crate::node_system::analysis::ResourceKey::new(function_path.as_str());
    let before = compile_resources_from_data(&data, Default::default())
        .unwrap()
        .versions[&key]
        .clone();
    data.graphs.get_mut(&function_path).unwrap().name = "Renamed Fingerprint".into();
    let after_name = compile_resources_from_data(&data, Default::default())
        .unwrap()
        .versions[&key]
        .clone();
    assert_ne!(before, after_name);

    let graph = data.graphs.get_mut(&function_path).unwrap();
    graph.document.revision = GraphRevision::new(1);
    let body_node = node("yssbi.constant.int64");
    graph.document.nodes.insert(body_node.id, body_node);
    let after_body = compile_resources_from_data(&data, Default::default())
        .unwrap()
        .versions[&key]
        .clone();

    assert_ne!(after_name, after_body);
}
