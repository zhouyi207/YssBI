use super::*;

#[cfg(test)]
mod startup_tests {
    use super::*;

    #[test]
    fn activation_generation_exhaustion_never_enters_or_reuses_a_transition() {
        let generation = Arc::new(std::sync::atomic::AtomicU64::new(u64::MAX - 1));

        let error = match ActivationGenerationTransition::begin(&generation) {
            Ok(_) => panic!("exhausted activation generation unexpectedly started"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            ProjectFilesystemError::ActivationGenerationExhausted
        ));
        assert_eq!(
            generation.load(std::sync::atomic::Ordering::Acquire),
            u64::MAX - 1
        );
    }

    #[test]
    fn publication_counter_exhaustion_has_zero_effects() {
        let mut publication = MutationPublication {
            project_instance_id: "project".into(),
            resource_revision: 7,
            authority_generation: u64::MAX,
            computation_settings_revision: 3,
        };

        assert_eq!(
            publication.allocate_resource_revision(),
            Err(ProjectFilesystemError::AuthorityGenerationExhausted)
        );
        assert_eq!(
            (
                publication.resource_revision,
                publication.authority_generation()
            ),
            (7, u64::MAX)
        );

        publication.resource_revision = u64::MAX;
        publication.authority_generation = 11;
        assert_eq!(
            publication.allocate_resource_revision(),
            Err(ProjectFilesystemError::PublicationRevisionExhausted)
        );
        assert_eq!(
            (
                publication.resource_revision,
                publication.authority_generation()
            ),
            (u64::MAX, 11)
        );
    }

    #[test]
    fn project_state_try_new_constructs_only_after_builtin_validation() {
        let state = ProjectState::try_new().unwrap();
        let store = state.project_store.read().unwrap();
        let node = crate::node_system::protocol::NodeTypeId::new("yssbi.constant.bool").unwrap();

        assert_eq!(
            store
                .node_registry
                .node_provider(&node)
                .map(crate::node_system::protocol::ProviderId::as_str),
            Some("yssbi.builtin")
        );
        assert!(state.project_data.read().unwrap().graphs.is_empty());
    }
}
#[cfg(test)]
mod execution_identity_tests {
    use super::*;
    use crate::node_system::runtime::{RunEvent, RunEventSink};
    use crate::project::GraphDocumentKind;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingRunEvents(Mutex<Vec<RunEvent>>);

    impl RunEventSink for RecordingRunEvents {
        fn record(&self, event: RunEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    #[test]
    fn execute_graph_rejects_stale_caller_before_run_registration() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-execution-entry-stale-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let graph_path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            graph_path.clone(),
            GraphResourceDocument::new("Main", GraphDocumentKind::Event),
        );
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project.clone());
        let stale_id = state.capture_project_session().unwrap().instance_id;
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
        let events = RecordingRunEvents::default();

        let error = state
            .execute_graph(
                &stale_id,
                &graph_path,
                &crate::node_system::plan::ExecutionDemand::Default,
                &events,
            )
            .unwrap_err();

        assert_eq!(
            error.kind(),
            ProjectExecutionErrorKind::StaleProjectLifecycle
        );
        assert!(events.0.lock().unwrap().is_empty());
        let store = state.project_store.read().unwrap();
        assert_eq!(store.runs.active_run_count(), 0);
        drop(store);
        let _ = std::fs::remove_dir_all(root);
    }
}
#[cfg(test)]
mod run_parameter_tests {
    use super::*;
    use crate::node_system::document::{
        DocumentNode, GraphDocument, NodeId, NodePosition, PortAddress,
    };
    use crate::node_system::plan::{
        CompiledParameterHandle, ExecutionPlan, ExecutionSemanticsVersion, GraphOutputRef,
        OperationStableId, PlannedKernel, PlannedOperation, PlannedPublication, PlannedRetry,
        WorkloadClass,
    };
    use crate::node_system::protocol::{CachePolicy, NodeTypeId, ParameterKey, PortKey, Value};
    use crate::project::GraphDocumentKind;
    use std::collections::BTreeMap;

    fn catalog_defaults(node_type: &NodeTypeId) -> BTreeMap<ParameterKey, serde_json::Value> {
        let registry = crate::node_system::catalog::build_builtin_node_system()
            .unwrap()
            .registry;
        registry
            .get(node_type)
            .unwrap()
            .protocol()
            .parameters
            .parameters
            .iter()
            .filter_map(|parameter| {
                let value = match &parameter.default_value.as_ref()?.value {
                    Value::Integer(value) => serde_json::json!(value),
                    Value::Unsigned(value) => serde_json::json!(value),
                    Value::String(value) => serde_json::json!(value),
                    other => panic!("unsupported test catalog default: {other:?}"),
                };
                Some((parameter.key.clone(), value))
            })
            .collect()
    }

    fn populate_plan_value_contracts(plan: &mut ExecutionPlan) {
        plan.value_contracts = plan
            .operations
            .iter()
            .flat_map(|operation| {
                operation
                    .inputs
                    .iter()
                    .map(|input| (input.value, input.contract.clone()))
                    .chain(
                        operation
                            .outputs
                            .iter()
                            .map(|output| (output.value, output.contract.clone())),
                    )
            })
            .collect();
    }

    fn parameter_plan(node: &DocumentNode, params: CompiledParameterHandle) -> ExecutionPlan {
        use crate::node_system::ProjectSessionId;
        use crate::node_system::analysis::{CompilationBasis, CompileId, CompileProvenance};
        use crate::node_system::document::{GraphResourcePath, GraphRevision};
        use crate::node_system::plan::StructuredControlRegion;
        use crate::node_system::registry::RegistryFingerprint;

        ExecutionPlan {
            provenance: CompileProvenance {
                project_session_id: ProjectSessionId::new("run-parameter-test"),
                graph_path: GraphResourcePath("events/test".into()),
                basis: CompilationBasis {
                    graph_revision: GraphRevision::new(1),
                    registry_fingerprint: RegistryFingerprint::from_bytes([1; 32]),
                    resource_versions: BTreeMap::new(),
                    resource_observations: BTreeMap::new(),
                },
                compile_id: CompileId::new(1),
            },
            value_count: 0,
            value_contracts: BTreeMap::new(),
            value_sources: Box::new([]),
            bound_values: BTreeMap::new(),
            operations: Box::new([PlannedOperation {
                stable_id: OperationStableId::new(format!("test.operation.{}", node.id)).unwrap(),
                source_node_id: node.id,
                source_node_type_id: node.node_type.clone(),
                kernel: PlannedKernel::Native(
                    crate::node_system::plan::KernelHandle::new("test.kernel").unwrap(),
                ),
                inputs: Box::new([]),
                outputs: Box::new([]),
                params,
                resource_dependencies: Box::new([]),
                cache_policy: CachePolicy::Disabled,
                semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
                workload: WorkloadClass::Cpu,
                retry: PlannedRetry::default(),
            }]),
            value_dependencies: Box::new([]),
            root_region: StructuredControlRegion::Sequence(Box::new([])),
            effect_dependencies: Box::new([]),
            relational_subplans: Box::new([]),
            resources: Box::new([]),
            results: Box::new([]),
            publications: Box::new([]),
        }
    }

    #[test]
    fn function_graph_replacement_changes_the_coherent_compile_resource_version() {
        let path = GraphResourcePath::new("functions/replaced.yssbi-function").unwrap();
        let analysis_path = crate::node_system::document::GraphResourcePath(path.as_str().into());
        let mut data = ProjectData::new();
        data.graphs.insert(
            path.clone(),
            GraphResourceDocument::new("Replaced", GraphDocumentKind::Function),
        );
        let first = compile_resources_from_data(&data, BTreeMap::new()).unwrap();
        let first_version = first
            .versions
            .get(&crate::node_system::analysis::ResourceKey::new(
                path.as_str(),
            ))
            .unwrap()
            .clone();

        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(91));
        data.graphs.get_mut(&path).unwrap().document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new("yssbi.project.function.entry").unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: BTreeMap::new(),
                user_label: None,
            },
        );
        let replaced = compile_resources_from_data(&data, BTreeMap::new()).unwrap();
        let replaced_version = replaced
            .versions
            .get(&crate::node_system::analysis::ResourceKey::new(
                path.as_str(),
            ))
            .unwrap();

        assert_ne!(&first_version, replaced_version);
        assert_eq!(
            replaced
                .function_graph_document(&analysis_path)
                .unwrap()
                .nodes
                .len(),
            1
        );
    }

    #[test]
    fn function_plan_publication_uses_only_the_compile_resource_snapshot() {
        let registry = crate::node_system::catalog::build_builtin_node_system()
            .unwrap()
            .registry;
        let session = crate::node_system::ProjectSessionId::new("coherent-run");
        let store = crate::node_system::runtime::FunctionPlanStore::new(session.clone(), 64);
        let resources = CompileResourceSnapshot {
            versions: crate::node_system::analysis::ResourceVersionSet::new(),
            resource_states: crate::node_system::analysis::ResourceObservationSet::new(),
            function_names: BTreeMap::new(),
            functions: BTreeMap::new(),
            function_graphs: BTreeMap::new(),
            variables: std::collections::HashMap::new(),
            database_names: BTreeMap::new(),
            database_schemas: BTreeMap::new(),
        };
        let mut parameters = crate::node_system::runtime::CompiledParameterStore::new();

        let generation = publish_function_plans(
            &registry,
            &store,
            &resources,
            None,
            session,
            &crate::node_system::compiler::CompileCancellationToken::new(),
            &crate::project::ProjectComputationSettings::default(),
            &mut parameters,
        )
        .unwrap();

        assert_eq!(generation.plan_count(), 0);
    }

    #[test]
    fn statistics_default_node_compiles_with_inherited_project_settings() {
        let node_type = NodeTypeId::new("yssbi.statistics.ols.fit").unwrap();
        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(4));
        let node = DocumentNode {
            id: node_id,
            node_type: node_type.clone(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: catalog_defaults(&node_type),
            user_label: None,
        };
        assert!(
            !node
                .parameters
                .contains_key(&ParameterKey::new("convergence_tolerance").unwrap())
        );
        assert!(
            !node
                .parameters
                .contains_key(&ParameterKey::new("missing_value_policy").unwrap())
        );
        let mut document = GraphDocument::default();
        document.nodes.insert(node_id, node.clone());
        let handle = CompiledParameterHandle::new("inherited-statistics-settings").unwrap();
        let plan = parameter_plan(&node, handle.clone());
        let mut store = crate::node_system::runtime::CompiledParameterStore::new();
        let project_settings = crate::project::ProjectComputationSettings {
            numeric: crate::project::NumericSettings {
                tolerance: crate::project::NumericTolerance {
                    absolute: 2e-6,
                    relative: 3e-5,
                },
            },
            missing_values: crate::project::MissingValueSettings {
                statistics: crate::project::StatisticalMissingValuePolicy::Reject,
            },
        };

        build_run_parameters(&mut store, &document, &plan, &project_settings).unwrap();
        let compiled = store
            .get::<crate::node_system::runtime::StatisticsKernelParameters>(&handle)
            .unwrap()
            .unwrap();
        assert_eq!(compiled.convergence_tolerance, 2e-6);
        assert_eq!(
            compiled.convergence_tolerance_source,
            crate::sci::models::regression::StatisticalSettingSource::Project
        );
        assert_eq!(
            compiled.missing_value_policy,
            crate::project::StatisticalMissingValuePolicy::Reject
        );
        assert_eq!(
            compiled.missing_value_policy_source,
            crate::sci::models::regression::StatisticalSettingSource::Project
        );
    }

    #[test]
    fn statistics_node_overrides_project_settings_in_compiled_parameters() {
        let node_type = NodeTypeId::new("yssbi.statistics.ols.fit").unwrap();
        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(2));
        let mut node_parameters = catalog_defaults(&node_type);
        node_parameters.insert(
            ParameterKey::new("convergence_tolerance").unwrap(),
            serde_json::json!(1e-7),
        );
        node_parameters.insert(
            ParameterKey::new("missing_value_policy").unwrap(),
            serde_json::json!("Reject"),
        );
        let node = DocumentNode {
            id: node_id,
            node_type,
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: node_parameters,
            user_label: None,
        };
        let mut document = GraphDocument::default();
        document.nodes.insert(node_id, node.clone());
        let handle = CompiledParameterHandle::new("statistics-settings").unwrap();
        let plan = parameter_plan(&node, handle.clone());
        let mut store = crate::node_system::runtime::CompiledParameterStore::new();
        let project_settings = crate::project::ProjectComputationSettings {
            numeric: crate::project::NumericSettings {
                tolerance: crate::project::NumericTolerance {
                    absolute: 1e-5,
                    relative: 1e-4,
                },
            },
            missing_values: crate::project::MissingValueSettings {
                statistics: crate::project::StatisticalMissingValuePolicy::Listwise,
            },
        };

        build_run_parameters(&mut store, &document, &plan, &project_settings).unwrap();
        let compiled = store
            .get::<crate::node_system::runtime::StatisticsKernelParameters>(&handle)
            .unwrap()
            .unwrap();
        assert_eq!(compiled.convergence_tolerance, 1e-7);
        assert_eq!(
            compiled.convergence_tolerance_source,
            crate::sci::models::regression::StatisticalSettingSource::Node
        );
        assert_eq!(
            compiled.missing_value_policy,
            crate::project::StatisticalMissingValuePolicy::Reject
        );
        assert_eq!(
            compiled.missing_value_policy_source,
            crate::sci::models::regression::StatisticalSettingSource::Node
        );
    }

    #[test]
    fn statistics_rejects_nonpositive_convergence_override() {
        let node_type = NodeTypeId::new("yssbi.statistics.ols.fit").unwrap();
        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(3));
        let mut node_parameters = catalog_defaults(&node_type);
        node_parameters.insert(
            ParameterKey::new("convergence_tolerance").unwrap(),
            serde_json::json!(0.0),
        );
        let node = DocumentNode {
            id: node_id,
            node_type,
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: node_parameters,
            user_label: None,
        };
        let mut document = GraphDocument::default();
        document.nodes.insert(node_id, node.clone());
        let plan = parameter_plan(
            &node,
            CompiledParameterHandle::new("invalid-statistics-settings").unwrap(),
        );
        let mut store = crate::node_system::runtime::CompiledParameterStore::new();

        let error = build_run_parameters(
            &mut store,
            &document,
            &plan,
            &crate::project::ProjectComputationSettings::default(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            "statistics convergence tolerance must be finite and greater than zero"
        );
    }

    #[test]
    fn adf_catalog_regression_builds_the_production_kernel_parameter() {
        let node_type = NodeTypeId::new("yssbi.statistics.adf.test").unwrap();
        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(1));
        let mut parameters = catalog_defaults(&node_type);
        assert_eq!(
            parameters.get(&ParameterKey::new("regression").unwrap()),
            Some(&serde_json::json!("constant"))
        );
        parameters.insert(
            ParameterKey::new("regression").unwrap(),
            serde_json::json!("none"),
        );
        let node = DocumentNode {
            id: node_id,
            node_type,
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters,
            user_label: None,
        };
        let mut document = GraphDocument::default();
        document.nodes.insert(node_id, node.clone());
        let handle = CompiledParameterHandle::new("adf").unwrap();
        let plan = parameter_plan(&node, handle.clone());
        let mut store = crate::node_system::runtime::CompiledParameterStore::new();

        build_run_parameters(
            &mut store,
            &document,
            &plan,
            &crate::project::ProjectComputationSettings::default(),
        )
        .unwrap();

        let parameters = store
            .get::<crate::node_system::runtime::StatisticsKernelParameters>(&handle)
            .unwrap()
            .unwrap();
        assert_eq!(parameters.regression.as_deref(), Some("none"));
        assert_eq!(parameters.trend, None);
    }

    struct NoResources;

    impl crate::node_system::runtime::ResourceProvider for NoResources {
        fn acquire(
            &self,
            _: &crate::node_system::plan::CompiledResourceRequirement,
        ) -> Result<
            Box<dyn crate::node_system::runtime::ResourceLease>,
            crate::node_system::runtime::ResourceError,
        > {
            unreachable!("statistics parameter test has no resources")
        }
    }

    struct NoFunctions;

    impl crate::node_system::runtime::FunctionPlanProvider for NoFunctions {
        fn get_function(
            &self,
            _: &crate::node_system::plan::FunctionPlanHandle,
        ) -> Result<
            Option<std::sync::Arc<crate::node_system::runtime::PublishedFunctionPlan>>,
            Box<str>,
        > {
            Ok(None)
        }
    }

    struct SeriesKernel(Value);

    impl crate::node_system::runtime::Kernel for SeriesKernel {
        fn execute(
            &self,
            _: &crate::node_system::runtime::KernelContext<'_>,
            _: &[crate::node_system::runtime::RuntimeValue],
        ) -> Result<
            Vec<crate::node_system::runtime::RuntimeValue>,
            crate::node_system::runtime::KernelError,
        > {
            let Value::List(values) = &self.0 else {
                return Err(crate::node_system::runtime::KernelError::new(
                    "test series fixture requires a list",
                ));
            };
            let values = values
                .iter()
                .map(|value| {
                    let decimal = match value {
                        Value::Integer(value) => {
                            crate::node_system::protocol::CanonicalDecimal::new(value.to_string())
                        }
                        Value::Unsigned(value) => {
                            crate::node_system::protocol::CanonicalDecimal::new(value.to_string())
                        }
                        Value::Decimal(value) => return Ok(Value::Decimal(value.clone())),
                        _ => {
                            return Err(crate::node_system::runtime::KernelError::new(
                                "test series fixture requires numeric values",
                            ));
                        }
                    }
                    .map_err(|error| {
                        crate::node_system::runtime::KernelError::new(error.to_string())
                    })?;
                    Ok(Value::Decimal(decimal))
                })
                .collect::<Result<Vec<_>, crate::node_system::runtime::KernelError>>()?;
            let artifact = crate::node_system::runtime::DataSeriesBuilder::new(
                crate::node_system::runtime::DataSeriesElementType::Float64,
            )
            .values(values)
            .build(crate::node_system::runtime::ArtifactKind::Collected)
            .map_err(|error| crate::node_system::runtime::KernelError::new(error.to_string()))?;
            Ok(vec![crate::node_system::runtime::RuntimeValue::Artifact(
                artifact,
            )])
        }
    }

    #[test]
    fn adf_regression_reaches_adapter_through_the_production_run_chain() {
        use crate::node_system::plan::{
            ControlStep, OperationIndex, PlanResult, PlannedInput, PlannedOutput,
            StructuredControlRegion, ValueRef,
        };
        use crate::node_system::protocol::{InputConsumption, OutputProduction};
        use crate::node_system::runtime::{CancellationToken, RunError, RunExecutor, RuntimeValue};

        let series_values =
            serde_json::json!([1, 1.4, 1.1, 1.8, 1.5, 2.2, 1.9, 2.6, 2.3, 3, 2.7, 3.4]);
        let series = [1.0, 1.4, 1.1, 1.8, 1.5, 2.2, 1.9, 2.6, 2.3, 3.0, 2.7, 3.4];
        let source_id = NodeId::from_uuid(uuid::Uuid::from_u128(10));
        let adf_id = NodeId::from_uuid(uuid::Uuid::from_u128(11));
        let adf_type = NodeTypeId::new("yssbi.statistics.adf.test").unwrap();
        let regression_key = ParameterKey::new("regression").unwrap();
        let mut adf_parameters = catalog_defaults(&adf_type);
        assert_eq!(
            adf_parameters.get(&regression_key),
            Some(&serde_json::json!("constant"))
        );
        adf_parameters.insert(regression_key.clone(), serde_json::json!("trend"));

        let source_node = DocumentNode {
            id: source_id,
            node_type: NodeTypeId::new("yssbi.test.adf.series").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        };
        let adf_node = DocumentNode {
            id: adf_id,
            node_type: adf_type.clone(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: adf_parameters,
            user_label: None,
        };
        let mut document = GraphDocument::default();
        document.nodes.insert(source_id, source_node.clone());
        document.nodes.insert(adf_id, adf_node.clone());

        let mut plan = parameter_plan(
            &adf_node,
            CompiledParameterHandle::new("adf-production-chain").unwrap(),
        );
        let series_contract = crate::node_system::plan::PlannedValueContract {
            kind: crate::node_system::plan::PlannedValueKind::DataSeries,
            type_expr: crate::node_system::protocol::numeric_data_series_type(),
        };
        plan.value_count = 2;
        plan.operations = Box::new([
            PlannedOperation {
                stable_id: OperationStableId::new(format!("test.operation.{source_id}")).unwrap(),
                source_node_id: source_id,
                source_node_type_id: source_node.node_type,
                kernel: PlannedKernel::Native(
                    crate::node_system::plan::KernelHandle::new("test.adf.series").unwrap(),
                ),
                inputs: Box::new([]),
                outputs: Box::new([PlannedOutput {
                    value: ValueRef::new(0),
                    contract: series_contract.clone(),
                    production: OutputProduction::FullyMaterialized,
                    public_output: None,
                    presentation: crate::node_system::plan::ResultPresentation::Inspector,
                }]),
                params: CompiledParameterHandle::new("adf-series").unwrap(),
                resource_dependencies: Box::new([]),
                cache_policy: CachePolicy::Disabled,
                semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
                workload: WorkloadClass::Cpu,
                retry: PlannedRetry::default(),
            },
            PlannedOperation {
                stable_id: OperationStableId::new(format!("test.operation.{adf_id}")).unwrap(),
                source_node_id: adf_id,
                source_node_type_id: adf_type,
                kernel: PlannedKernel::Native(
                    crate::node_system::plan::KernelHandle::new("yssbi.statistics.adf.test")
                        .unwrap(),
                ),
                inputs: Box::new([PlannedInput {
                    value: ValueRef::new(0),
                    contract: series_contract,
                    consumption: InputConsumption::FullyMaterialized,
                    bound_value: None,
                }]),
                outputs: Box::new([PlannedOutput {
                    value: ValueRef::new(1),
                    contract: crate::node_system::plan::PlannedValueContract::opaque(),
                    production: OutputProduction::FullyMaterialized,
                    public_output: None,
                    presentation: crate::node_system::plan::ResultPresentation::Inspector,
                }]),
                params: CompiledParameterHandle::new("adf-production-chain").unwrap(),
                resource_dependencies: Box::new([]),
                cache_policy: CachePolicy::Disabled,
                semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
                workload: WorkloadClass::Cpu,
                retry: PlannedRetry::default(),
            },
        ]);
        plan.root_region = StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ]));
        let adf_output = GraphOutputRef {
            graph_path: plan.provenance.graph_path.clone(),
            port: PortAddress::declared(adf_id, PortKey::new("result").unwrap()),
        };
        plan.results = Box::new([PlanResult {
            name: "adf".into(),
            output: adf_output.clone(),
            value: ValueRef::new(1),
        }]);
        plan.publications = Box::new([PlannedPublication::GraphResult {
            name: "adf".into(),
            output: adf_output,
            value: ValueRef::new(1),
        }]);
        populate_plan_value_contracts(&mut plan);

        let mut kernels = crate::node_system::runtime::build_builtin_kernel_registry();
        kernels
            .register(
                crate::node_system::plan::KernelHandle::new("test.adf.series").unwrap(),
                SeriesKernel(json_to_protocol_value(&series_values).unwrap()),
            )
            .unwrap();
        let run = |document: &GraphDocument| {
            let mut store = crate::node_system::runtime::CompiledParameterStore::new();
            build_run_parameters(
                &mut store,
                document,
                &plan,
                &crate::project::ProjectComputationSettings::default(),
            )
            .unwrap();
            RunExecutor::new(
                &kernels,
                &NoResources,
                &NoFunctions,
                crate::node_system::runtime::ResultStore::new(),
                std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
            )
            .with_compiled_parameters(&store)
            .run(&plan, CancellationToken::new())
        };

        let trend_result = run(&document).unwrap();
        let RuntimeValue::Scalar(Value::Object(actual)) =
            &trend_result.value_for_test("adf").unwrap()
        else {
            panic!("ADF result must be an object");
        };
        let expected_trend =
            crate::sci::api::node_statistics::augmented_dickey_fuller(&series, 1, "trend").unwrap();
        let constant_result =
            crate::sci::api::node_statistics::augmented_dickey_fuller(&series, 1, "constant")
                .unwrap();
        let protocol_number = |value: &Value| match value {
            Value::Integer(value) => *value as f64,
            Value::Unsigned(value) => *value as f64,
            Value::Decimal(value) => value.as_str().parse::<f64>().unwrap(),
            other => panic!("expected numeric protocol value, got {other:?}"),
        };
        let actual_statistic = protocol_number(&actual["statistic"]);
        let trend_statistic = expected_trend["statistic"].as_f64().unwrap();
        let constant_statistic = constant_result["statistic"].as_f64().unwrap();
        assert!((actual_statistic - trend_statistic).abs() < f64::EPSILON);
        assert!((actual_statistic - constant_statistic).abs() > f64::EPSILON);

        document
            .nodes
            .get_mut(&adf_id)
            .unwrap()
            .parameters
            .insert(regression_key, serde_json::json!("unexpected"));
        let error = run(&document).unwrap_err();
        assert!(matches!(
            error,
            RunError::KernelFailed { operation, ref message, .. }
                if operation == OperationIndex::new(1)
                    && message.as_ref() == "unsupported ADF regression 'unexpected'"
        ));
    }

    #[test]
    fn var_summary_catalog_lags_reach_the_production_kernel() {
        use crate::node_system::plan::{
            ControlStep, OperationIndex, PlanResult, PlannedInput, PlannedOutput,
            StructuredControlRegion, ValueRef,
        };
        use crate::node_system::protocol::{InputConsumption, OutputProduction};
        use crate::node_system::runtime::{CancellationToken, RunExecutor, RuntimeValue};

        let var_type = NodeTypeId::new("yssbi.statistics.var.summary").unwrap();
        let mut var_parameters = catalog_defaults(&var_type);
        let lags_key = ParameterKey::new("lags").unwrap();
        assert_eq!(var_parameters.get(&lags_key), Some(&serde_json::json!(1)));
        var_parameters.insert(lags_key, serde_json::json!(2));

        let node_specs = [
            (1_u128, "yssbi.test.series.a"),
            (2_u128, "yssbi.test.series.b"),
        ];
        let mut document = GraphDocument::default();
        let mut constant_ids = Vec::new();
        for (raw_id, node_type) in node_specs {
            let id = NodeId::from_uuid(uuid::Uuid::from_u128(raw_id));
            constant_ids.push(id);
            document.nodes.insert(
                id,
                DocumentNode {
                    id,
                    node_type: NodeTypeId::new(node_type).unwrap(),
                    position: NodePosition { x: 0.0, y: 0.0 },
                    parameters: BTreeMap::new(),
                    user_label: None,
                },
            );
        }
        let var_id = NodeId::from_uuid(uuid::Uuid::from_u128(3));
        document.nodes.insert(
            var_id,
            DocumentNode {
                id: var_id,
                node_type: var_type.clone(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: var_parameters,
                user_label: None,
            },
        );

        let mut plan = parameter_plan(
            document.nodes.get(&var_id).unwrap(),
            CompiledParameterHandle::new("var").unwrap(),
        );
        let constant_operation = |index: usize| PlannedOperation {
            stable_id: OperationStableId::new(format!("test.operation.{}", constant_ids[index]))
                .unwrap(),
            source_node_id: constant_ids[index],
            source_node_type_id: NodeTypeId::new(format!("yssbi.test.series.{index}")).unwrap(),
            kernel: PlannedKernel::Native(
                crate::node_system::plan::KernelHandle::new(format!("test.series.{index}"))
                    .unwrap(),
            ),
            inputs: Box::new([]),
            outputs: Box::new([PlannedOutput {
                value: ValueRef::new(index as u32),
                contract: crate::node_system::plan::PlannedValueContract::opaque(),
                production: OutputProduction::FullyMaterialized,
                public_output: None,
                presentation: crate::node_system::plan::ResultPresentation::Inspector,
            }]),
            params: CompiledParameterHandle::new(format!("series-{index}")).unwrap(),
            resource_dependencies: Box::new([]),
            cache_policy: CachePolicy::Disabled,
            semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
            workload: WorkloadClass::Cpu,
            retry: PlannedRetry::default(),
        };
        let var_operation = PlannedOperation {
            stable_id: OperationStableId::new(format!("test.operation.{var_id}")).unwrap(),
            source_node_id: var_id,
            source_node_type_id: var_type,
            kernel: PlannedKernel::Native(
                crate::node_system::plan::KernelHandle::new("yssbi.statistics.var.summary")
                    .unwrap(),
            ),
            inputs: Box::new([
                PlannedInput {
                    value: ValueRef::new(0),
                    contract: crate::node_system::plan::PlannedValueContract::opaque(),
                    consumption: InputConsumption::FullyMaterialized,
                    bound_value: None,
                },
                PlannedInput {
                    value: ValueRef::new(1),
                    contract: crate::node_system::plan::PlannedValueContract::opaque(),
                    consumption: InputConsumption::FullyMaterialized,
                    bound_value: None,
                },
            ]),
            outputs: Box::new([
                PlannedOutput {
                    value: ValueRef::new(2),
                    contract: crate::node_system::plan::PlannedValueContract::opaque(),
                    production: OutputProduction::FullyMaterialized,
                    public_output: None,
                    presentation: crate::node_system::plan::ResultPresentation::Inspector,
                },
                PlannedOutput {
                    value: ValueRef::new(3),
                    contract: crate::node_system::plan::PlannedValueContract::opaque(),
                    production: OutputProduction::FullyMaterialized,
                    public_output: None,
                    presentation: crate::node_system::plan::ResultPresentation::Inspector,
                },
            ]),
            params: CompiledParameterHandle::new("var").unwrap(),
            resource_dependencies: Box::new([]),
            cache_policy: CachePolicy::Disabled,
            semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
            workload: WorkloadClass::Cpu,
            retry: PlannedRetry::default(),
        };
        plan.value_count = 4;
        plan.operations = Box::new([constant_operation(0), constant_operation(1), var_operation]);
        plan.root_region = StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
            ControlStep::Operation(OperationIndex::new(2)),
        ]));
        let var_output = GraphOutputRef {
            graph_path: plan.provenance.graph_path.clone(),
            port: PortAddress::declared(var_id, PortKey::new("summary").unwrap()),
        };
        plan.results = Box::new([PlanResult {
            name: "var".into(),
            output: var_output.clone(),
            value: ValueRef::new(2),
        }]);
        plan.publications = Box::new([PlannedPublication::GraphResult {
            name: "var".into(),
            output: var_output,
            value: ValueRef::new(2),
        }]);
        populate_plan_value_contracts(&mut plan);
        let mut store = crate::node_system::runtime::CompiledParameterStore::new();
        build_run_parameters(
            &mut store,
            &document,
            &plan,
            &crate::project::ProjectComputationSettings::default(),
        )
        .unwrap();

        let mut kernels = crate::node_system::runtime::build_builtin_kernel_registry();
        for (index, values) in [
            serde_json::json!([1, 1.2, 0.9, 1.1, 1.4, 1, 0.8, 1.3, 1.1, 0.9, 1.2, 1.5]),
            serde_json::json!([0.5, 0.7, 0.6, 0.9, 0.8, 1, 0.7, 0.6, 0.9, 1.1, 0.8, 0.7]),
        ]
        .into_iter()
        .enumerate()
        {
            kernels
                .register(
                    crate::node_system::plan::KernelHandle::new(format!("test.series.{index}"))
                        .unwrap(),
                    SeriesKernel(json_to_protocol_value(&values).unwrap()),
                )
                .unwrap();
        }
        let result = RunExecutor::new(
            &kernels,
            &NoResources,
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .with_compiled_parameters(&store)
        .run(&plan, CancellationToken::new())
        .unwrap();
        let RuntimeValue::Scalar(Value::Object(result)) = &result.value_for_test("var").unwrap()
        else {
            panic!("VAR result must be an object");
        };
        let Value::List(coefficients) = &result["coefficients"] else {
            panic!("VAR coefficients must be grouped by equation");
        };
        let Value::List(labels) = &result["coef_labels"] else {
            panic!("VAR coefficient labels must be grouped by equation");
        };
        for (coefficients, labels) in coefficients.iter().zip(labels) {
            let Value::List(coefficients) = coefficients else {
                panic!("equation coefficients must be a list");
            };
            let Value::List(labels) = labels else {
                panic!("equation labels must be a list");
            };
            assert_eq!(coefficients.len(), 5);
            assert!(
                labels.iter().any(|label| {
                    matches!(label, Value::String(label) if label.contains("L2."))
                })
            );
        }
    }
}
