//! Pure, immutable execution-plan contracts.
//!
//! This module deliberately contains no registry lookup, graph document access,
//! I/O, acquired resources, or run state. All compact indices are local to one
//! `ExecutionPlan` and serialize only as part of that immutable plan product.

mod model;
mod result_presentation;
mod validation;

pub use model::*;
pub(crate) use result_presentation::presentation_for_output;
pub use result_presentation::{ResultPlotKind, ResultPresentation, ResultReportKind};
pub(crate) use validation::PlanSourceFacts;
pub use validation::{PlanValidationError, PlanValidationErrors};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_document::{GraphResourcePath, GraphRevision, NodeId, PortAddress};
    use crate::node_system::ProjectSessionId;
    use crate::node_system::analysis::{
        CompilationBasis, CompileId, CompileProvenance, ResourceVersionSet,
    };
    use crate::node_system::protocol::{
        CachePolicy, InputConsumption, NodeTypeId, OutputProduction, PortKey, RetryPolicy,
        TypeExpr, TypeId, Value, data_series_type,
    };
    use crate::node_system::registry::RegistryFingerprint;

    fn id<T>(value: &str, constructor: impl FnOnce(Box<str>) -> Result<T, InvalidPlanId>) -> T {
        constructor(value.into()).unwrap()
    }

    #[test]
    fn attempt_id_rejects_zero() {
        assert_eq!(AttemptId::try_new(0), Err(InvalidAttemptId));
        assert_eq!(AttemptId::try_new(1).unwrap(), AttemptId::initial());
    }

    fn operation(output: u32) -> PlannedOperation {
        PlannedOperation {
            stable_id: OperationStableId::new(format!("test.operation.{output}")).unwrap(),
            source_node_id: NodeId::from_uuid(uuid::Uuid::nil()),
            source_node_type_id: NodeTypeId::new("yssbi.test.node").unwrap(),
            kernel: PlannedKernel::Native(id("kernel.test", KernelHandle::new)),
            inputs: Box::new([]),
            outputs: Box::new([PlannedOutput {
                value: ValueRef::new(output),
                contract: crate::node_system::plan::PlannedValueContract::opaque(),
                production: OutputProduction::FullyMaterialized,
                public_output: None,
                presentation: ResultPresentation::Inspector,
            }]),
            params: id("params-1", CompiledParameterHandle::new),
            resource_dependencies: Box::new([]),
            cache_policy: CachePolicy::Disabled,
            semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
            workload: WorkloadClass::Cpu,
            retry: PlannedRetry::default(),
        }
    }

    fn valid_plan() -> ExecutionPlan {
        let result_output = GraphOutputRef {
            graph_path: GraphResourcePath::new("events/test").unwrap(),
            port: PortAddress::declared(
                NodeId::from_uuid(uuid::Uuid::nil()),
                PortKey::new("result").unwrap(),
            ),
        };
        ExecutionPlan {
            provenance: CompileProvenance {
                project_session_id: ProjectSessionId::new("test-session"),
                graph_path: GraphResourcePath::new("events/test").unwrap(),
                basis: CompilationBasis {
                    graph_revision: GraphRevision::new(7),
                    registry_fingerprint: RegistryFingerprint::from_bytes([1; 32]),
                    resource_versions: ResourceVersionSet::new(),
                    resource_observations: Default::default(),
                },
                compile_id: CompileId::new(1),
            },
            value_count: 8,
            operations: Box::new([operation(0), operation(1)]),
            value_contracts: (0..8)
                .map(|value| (ValueRef::new(value), PlannedValueContract::opaque()))
                .collect(),
            value_sources: Box::new([
                PlanValueSource::ExternalInput(
                    ValueRef::new(3),
                    OutputProduction::FullyMaterialized,
                ),
                PlanValueSource::ExternalInput(
                    ValueRef::new(4),
                    OutputProduction::FullyMaterialized,
                ),
                PlanValueSource::ControlProduced(
                    ValueRef::new(2),
                    OutputProduction::FullyMaterialized,
                ),
                PlanValueSource::ControlProduced(
                    ValueRef::new(5),
                    OutputProduction::FullyMaterialized,
                ),
                PlanValueSource::ControlProduced(
                    ValueRef::new(6),
                    OutputProduction::FullyMaterialized,
                ),
                PlanValueSource::ControlProduced(
                    ValueRef::new(7),
                    OutputProduction::FullyMaterialized,
                ),
            ]),
            bound_values: Default::default(),
            value_dependencies: Box::new([ValueDependency {
                source: ValueRef::new(0),
                destination: ValueRef::new(1),
            }]),
            root_region: StructuredControlRegion::Sequence(Box::new([
                ControlStep::Operation(OperationIndex::new(0)),
                ControlStep::Region(Box::new(StructuredControlRegion::If {
                    condition: ValueRef::new(0),
                    then_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
                    else_region: Box::new(StructuredControlRegion::Call {
                        target: id("function.test", FunctionPlanHandle::new),
                        arguments: Box::new([CallArgumentBinding {
                            caller_source: ValueRef::new(0),
                            callee_destination: ValueRef::new(1),
                        }]),
                        results: Box::new([CallResultBinding {
                            callee_source: ValueRef::new(0),
                            caller_destination: ValueRef::new(7),
                            production: Some(OutputProduction::FullyMaterialized),
                        }]),
                        mandatory: true,
                    }),
                    results: Box::new([BranchResultBinding {
                        destination: ValueRef::new(2),
                        then_source: ValueRef::new(3),
                        else_source: ValueRef::new(4),
                        production: Some(OutputProduction::FullyMaterialized),
                    }]),
                })),
                ControlStep::Region(Box::new(StructuredControlRegion::Loop {
                    body: Box::new(StructuredControlRegion::Sequence(Box::new([
                        ControlStep::Operation(OperationIndex::new(1)),
                    ]))),
                    carried: Box::new([LoopCarriedBinding {
                        body_input: ValueRef::new(5),
                        initial_source: ValueRef::new(0),
                        next_source: ValueRef::new(1),
                        result: ValueRef::new(6),
                        production: Some(OutputProduction::FullyMaterialized),
                    }]),
                    continue_condition: ValueRef::new(0),
                    max_iterations: 10,
                })),
            ])),
            effect_dependencies: Box::new([EffectDependency {
                before: OperationIndex::new(0),
                after: OperationIndex::new(1),
            }]),
            relational_subplans: Box::new([]),
            resources: Box::new([CompiledResourceRequirement {
                resource: id("database.main", ResourceId::new),
                kind: ResourceKind::DatabaseConnection,
                access: ResourceAccess::Shared,
                optional: false,
            }]),
            results: Box::new([PlanResult {
                name: "result".into(),
                output: result_output.clone(),
                value: ValueRef::new(6),
            }]),
            publications: Box::new([PlannedPublication::GraphResult {
                name: "result".into(),
                output: result_output,
                value: ValueRef::new(6),
            }]),
        }
    }

    #[test]
    fn validation_rejects_duplicate_public_output_identity() {
        let mut plan = valid_plan();
        let public_output = GraphOutputRef {
            graph_path: plan.provenance.graph_path.clone(),
            port: PortAddress::declared(
                plan.operations[0].source_node_id,
                PortKey::new("shared").unwrap(),
            ),
        };
        plan.operations[0].outputs[0].public_output = Some(public_output.clone());
        plan.operations[1].source_node_id = plan.operations[0].source_node_id;
        plan.operations[1].outputs[0].public_output = Some(public_output.clone());

        assert!(plan.validate().unwrap_err().0.iter().any(|error| {
            matches!(error, PlanValidationError::DuplicatePublicOutput(output) if output == &public_output)
        }));
    }

    #[test]
    fn validation_rejects_wrong_public_output_graph_and_node() {
        let mut wrong_graph = valid_plan();
        wrong_graph.operations[0].outputs[0].public_output = Some(GraphOutputRef {
            graph_path: GraphResourcePath::new("events/other").unwrap(),
            port: PortAddress::declared(
                wrong_graph.operations[0].source_node_id,
                PortKey::new("result").unwrap(),
            ),
        });
        assert!(wrong_graph.validate().unwrap_err().0.iter().any(|error| {
            matches!(error, PlanValidationError::InvalidPublicOutput { operation, .. } if operation.index() == 0)
        }));

        let mut wrong_node = valid_plan();
        wrong_node.operations[0].outputs[0].public_output = Some(GraphOutputRef {
            graph_path: wrong_node.provenance.graph_path.clone(),
            port: PortAddress::declared(
                NodeId::from_uuid(uuid::Uuid::from_u128(99)),
                PortKey::new("result").unwrap(),
            ),
        });
        assert!(wrong_node.validate().unwrap_err().0.iter().any(|error| {
            matches!(error, PlanValidationError::InvalidPublicOutput { operation, .. } if operation.index() == 0)
        }));
    }

    #[test]
    fn validation_rejects_non_data_public_output_when_result_fact_is_exact() {
        let mut plan = valid_plan();
        let result = plan.results[0].clone();
        plan.operations[1].outputs[0].value = result.value;
        plan.operations[1].outputs[0].public_output = Some(GraphOutputRef {
            graph_path: result.output.graph_path.clone(),
            port: PortAddress::declared(
                plan.operations[1].source_node_id,
                PortKey::new("then").unwrap(),
            ),
        });

        assert!(plan.validate().unwrap_err().0.iter().any(|error| {
            matches!(error, PlanValidationError::PublicOutputResultMismatch { value, .. } if value == &result.value)
        }));
    }

    #[test]
    fn effective_cache_policy_validation_rejects_duplicate_operation_stable_ids() {
        let mut plan = valid_plan();
        plan.operations[1].stable_id = plan.operations[0].stable_id.clone();

        assert!(plan.validate().unwrap_err().0.iter().any(|error| {
            matches!(
                error,
                PlanValidationError::DuplicateOperationStableId {
                    stable_id,
                    first,
                    duplicate,
                } if stable_id == &plan.operations[0].stable_id
                    && first.index() == 0
                    && duplicate.index() == 1
            )
        }));
    }

    #[test]
    fn rejects_invalid_planned_retry_policy() {
        let mut plan = valid_plan();
        plan.operations[0].retry = PlannedRetry {
            idempotent: true,
            policy: Some(RetryPolicy {
                max_attempts: std::num::NonZeroU32::new(2).unwrap(),
                initial_backoff: std::time::Duration::from_millis(20),
                max_backoff: std::time::Duration::from_millis(10),
            }),
        };

        assert!(plan.validate().unwrap_err().0.iter().any(|error| {
            matches!(
                error,
                PlanValidationError::InvalidRetryPolicy { operation }
                    if operation.index() == 0
            )
        }));
    }

    #[test]
    fn rejects_duplicate_stable_outputs_names_and_invalid_internal_values() {
        let mut duplicate_output = valid_plan();
        duplicate_output.results = Box::new([
            duplicate_output.results[0].clone(),
            PlanResult {
                name: "other".into(),
                output: duplicate_output.results[0].output.clone(),
                value: ValueRef::new(6),
            },
        ]);
        assert!(
            duplicate_output
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| matches!(error, PlanValidationError::DuplicateResultOutput(_)))
        );

        let mut duplicate_name = valid_plan();
        duplicate_name.results = Box::new([
            duplicate_name.results[0].clone(),
            PlanResult {
                name: "result".into(),
                output: GraphOutputRef {
                    graph_path: GraphResourcePath::new("events/test").unwrap(),
                    port: PortAddress::declared(
                        NodeId::from_uuid(uuid::Uuid::nil()),
                        PortKey::new("other").unwrap(),
                    ),
                },
                value: ValueRef::new(6),
            },
        ]);
        assert!(duplicate_name.validate().unwrap_err().0.iter().any(|error| {
            matches!(error, PlanValidationError::DuplicateResultName(name) if name.as_ref() == "result")
        }));

        let mut invalid_value = valid_plan();
        invalid_value.results[0].value = ValueRef::new(invalid_value.value_count);
        assert!(invalid_value.validate().unwrap_err().0.iter().any(|error| {
            matches!(
                error,
                PlanValidationError::ValueOutOfBounds {
                    context: "plan result",
                    ..
                }
            )
        }));
    }

    #[test]
    fn publications_require_exact_available_results_and_one_non_mixed_mode() {
        let result = valid_plan().results[0].clone();

        let mut missing = valid_plan();
        missing.publications = Box::new([]);
        assert!(
            missing
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| matches!(
                    error,
                    PlanValidationError::GraphPublicationCountMismatch {
                        publications: 0,
                        results: 1,
                    }
                ))
        );

        let mut unexpected = valid_plan();
        unexpected.results = Box::new([]);
        assert!(
            unexpected
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| matches!(
                    error,
                    PlanValidationError::GraphPublicationCountMismatch {
                        publications: 1,
                        results: 0,
                    }
                ))
        );

        let mut graph = valid_plan();
        graph.publications = Box::new([PlannedPublication::GraphResult {
            name: result.name.clone(),
            output: result.output.clone(),
            value: result.value,
        }]);
        graph
            .validate()
            .expect("exact graph result publication is valid");

        let mut preview = valid_plan();
        preview.publications = Box::new([PlannedPublication::PinPreview {
            output: result.output.clone(),
            generation: 17,
            value: result.value,
        }]);
        preview
            .validate()
            .expect("exact preview publication is valid");

        let mut mismatched = valid_plan();
        mismatched.publications = Box::new([PlannedPublication::GraphResult {
            name: "wrong".into(),
            output: result.output.clone(),
            value: result.value,
        }]);
        assert!(mismatched.validate().is_err());

        let mut out_of_range = valid_plan();
        out_of_range.publications = Box::new([PlannedPublication::PinPreview {
            output: result.output.clone(),
            generation: 17,
            value: ValueRef::new(out_of_range.value_count),
        }]);
        assert!(out_of_range.validate().is_err());

        let mut unavailable = valid_plan();
        unavailable.results[0].value = ValueRef::new(1);
        unavailable.publications = Box::new([PlannedPublication::GraphResult {
            name: unavailable.results[0].name.clone(),
            output: unavailable.results[0].output.clone(),
            value: ValueRef::new(1),
        }]);
        assert!(
            unavailable
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| matches!(
                    error,
                    PlanValidationError::MissingPublicationSource(value)
                        if *value == ValueRef::new(1)
                ))
        );

        let mut duplicate = graph.clone();
        duplicate.publications = vec![
            duplicate.publications[0].clone(),
            duplicate.publications[0].clone(),
        ]
        .into_boxed_slice();
        assert!(duplicate.validate().is_err());

        let mut mixed = graph.clone();
        mixed.publications = Box::new([
            mixed.publications[0].clone(),
            PlannedPublication::PinPreview {
                output: result.output.clone(),
                generation: 17,
                value: result.value,
            },
        ]);
        assert!(mixed.validate().is_err());

        let mut multiple_previews = preview.clone();
        multiple_previews.publications = vec![
            multiple_previews.publications[0].clone(),
            multiple_previews.publications[0].clone(),
        ]
        .into_boxed_slice();
        assert!(multiple_previews.validate().is_err());

        let mut unsafe_generation = valid_plan();
        unsafe_generation.publications = Box::new([PlannedPublication::PinPreview {
            output: result.output,
            generation: 9_007_199_254_740_992,
            value: result.value,
        }]);
        assert!(unsafe_generation.validate().is_err());
    }

    fn valid_bridged_plan() -> (ExecutionPlan, ()) {
        let mut plan = valid_plan();
        let producer = id("fragment.producer", RelationalFragmentId::new);
        let consumer = id("fragment.consumer", RelationalFragmentId::new);
        plan.relational_subplans = Box::new([
            RelationalSubplan {
                backend: id("relational.test", RelationalBackendId::new),
                compiled_plan: CompiledRelationalPlan {
                    fragment_order: Box::new([producer.clone()]),
                    operators: Box::new([RelationalOperator::Source {
                        resource: id("database.main", ResourceId::new),
                        relation: "items".into(),
                    }]),
                    fragment_roots: Box::new([RelationalFragmentRoot {
                        fragment: producer,
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    roots: Box::new([RelationalOperatorIndex::new(0)]),
                    pushdown_hints: Box::new([]),
                },
            },
            RelationalSubplan {
                backend: id("relational.test", RelationalBackendId::new),
                compiled_plan: CompiledRelationalPlan {
                    fragment_order: Box::new([consumer.clone()]),
                    operators: Box::new([RelationalOperator::Input {
                        name: "input".into(),
                    }]),
                    fragment_roots: Box::new([RelationalFragmentRoot {
                        fragment: consumer,
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    roots: Box::new([RelationalOperatorIndex::new(0)]),
                    pushdown_hints: Box::new([]),
                },
            },
        ]);
        plan.operations[0].kernel = PlannedKernel::Relational(RelationalSubplanIndex::new(0));
        plan.operations[1].kernel = PlannedKernel::Relational(RelationalSubplanIndex::new(1));
        (plan, ())
    }

    #[test]
    fn execution_plan_round_trips_complete_object_graph() {
        let plan = valid_bridged_plan().0;

        let serialized = serde_json::to_vec(&plan).expect("execution plan should serialize");
        let deserialized: ExecutionPlan =
            serde_json::from_slice(&serialized).expect("execution plan should deserialize");

        assert_eq!(deserialized, plan);
    }

    #[test]
    fn validates_all_structured_region_variants() {
        assert_eq!(valid_plan().validate(), Ok(()));
    }

    #[test]
    fn rejects_invalid_branch_and_loop_value_bindings() {
        fn has_error(plan: &ExecutionPlan, name: &str) -> bool {
            plan.validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| format!("{error:?}").starts_with(name))
        }

        let mut duplicate_branch_destination = valid_plan();
        let StructuredControlRegion::Sequence(steps) =
            &mut duplicate_branch_destination.root_region
        else {
            unreachable!()
        };
        let ControlStep::Region(branch) = &mut steps[1] else {
            unreachable!()
        };
        let StructuredControlRegion::If { results, .. } = branch.as_mut() else {
            unreachable!()
        };
        *results = Box::new([results[0], results[0]]);
        assert!(has_error(
            &duplicate_branch_destination,
            "DuplicateBranchResultDestination"
        ));

        let mut aliased_branch_roles = valid_plan();
        let StructuredControlRegion::Sequence(steps) = &mut aliased_branch_roles.root_region else {
            unreachable!()
        };
        let ControlStep::Region(branch) = &mut steps[1] else {
            unreachable!()
        };
        let StructuredControlRegion::If { results, .. } = branch.as_mut() else {
            unreachable!()
        };
        results[0].then_source = results[0].destination;
        assert!(has_error(&aliased_branch_roles, "InvalidBranchResultRoles"));

        let mut missing_loop_binding = valid_plan();
        let StructuredControlRegion::Sequence(steps) = &mut missing_loop_binding.root_region else {
            unreachable!()
        };
        let ControlStep::Region(loop_region) = &mut steps[2] else {
            unreachable!()
        };
        let StructuredControlRegion::Loop { carried, .. } = loop_region.as_mut() else {
            unreachable!()
        };
        *carried = Box::new([]);
        assert!(has_error(
            &missing_loop_binding,
            "MissingLoopCarriedBinding"
        ));

        let mut duplicate_loop_destinations = valid_plan();
        let StructuredControlRegion::Sequence(steps) = &mut duplicate_loop_destinations.root_region
        else {
            unreachable!()
        };
        let ControlStep::Region(loop_region) = &mut steps[2] else {
            unreachable!()
        };
        let StructuredControlRegion::Loop { carried, .. } = loop_region.as_mut() else {
            unreachable!()
        };
        *carried = Box::new([carried[0], carried[0]]);
        assert!(has_error(
            &duplicate_loop_destinations,
            "DuplicateLoopBodyInputDestination"
        ));
        assert!(has_error(
            &duplicate_loop_destinations,
            "DuplicateLoopResultDestination"
        ));

        let mut missing_control_destination = valid_plan();
        missing_control_destination.value_sources = Box::new([]);
        assert!(has_error(
            &missing_control_destination,
            "MissingControlProducedDeclaration"
        ));
    }

    #[test]
    fn call_validation_treats_callee_refs_as_opaque_cross_frame_values() {
        let mut plan = valid_plan();
        let StructuredControlRegion::Sequence(steps) = &mut plan.root_region else {
            unreachable!()
        };
        let ControlStep::Region(branch) = &mut steps[1] else {
            unreachable!()
        };
        let StructuredControlRegion::If { else_region, .. } = branch.as_mut() else {
            unreachable!()
        };
        let StructuredControlRegion::Call {
            arguments, results, ..
        } = else_region.as_mut()
        else {
            unreachable!()
        };
        arguments[0].callee_destination = ValueRef::new(1_000);
        results[0].callee_source = ValueRef::new(2_000);

        assert_eq!(plan.validate(), Ok(()));
    }

    #[test]
    fn call_validation_keeps_caller_side_bounds_sources_and_producers() {
        fn call_bindings(
            plan: &mut ExecutionPlan,
        ) -> (
            &mut Box<[CallArgumentBinding]>,
            &mut Box<[CallResultBinding]>,
        ) {
            let StructuredControlRegion::Sequence(steps) = &mut plan.root_region else {
                unreachable!()
            };
            let ControlStep::Region(branch) = &mut steps[1] else {
                unreachable!()
            };
            let StructuredControlRegion::If { else_region, .. } = branch.as_mut() else {
                unreachable!()
            };
            let StructuredControlRegion::Call {
                arguments, results, ..
            } = else_region.as_mut()
            else {
                unreachable!()
            };
            (arguments, results)
        }

        let mut unsourced_argument = valid_plan();
        unsourced_argument.value_count = 9;
        call_bindings(&mut unsourced_argument).0[0].caller_source = ValueRef::new(8);
        assert!(
            unsourced_argument
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| {
                    matches!(
                        error,
                        PlanValidationError::MissingStructuredBindingSource {
                            context: "call argument source",
                            value,
                        } if *value == ValueRef::new(8)
                    )
                })
        );

        let mut out_of_bounds_result = valid_plan();
        call_bindings(&mut out_of_bounds_result).1[0].caller_destination = ValueRef::new(99);
        assert!(
            out_of_bounds_result
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| {
                    matches!(
                        error,
                        PlanValidationError::ValueOutOfBounds {
                            context: "call result destination",
                            value,
                            ..
                        } if *value == ValueRef::new(99)
                    )
                })
        );

        let mut undeclared_result = valid_plan();
        undeclared_result.value_sources = undeclared_result
            .value_sources
            .into_vec()
            .into_iter()
            .filter(|source| {
                *source
                    != PlanValueSource::ControlProduced(
                        ValueRef::new(7),
                        OutputProduction::FullyMaterialized,
                    )
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        assert!(
            undeclared_result
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| {
                    matches!(
                        error,
                        PlanValidationError::MissingControlProducedDeclaration {
                            value,
                            producer: "call result",
                        } if *value == ValueRef::new(7)
                    )
                })
        );

        let mut duplicate_result = valid_plan();
        call_bindings(&mut duplicate_result).1[0].caller_destination = ValueRef::new(2);
        assert!(
            duplicate_result
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| {
                    matches!(
                        error,
                        PlanValidationError::DuplicateStructuredControlProducer {
                            value,
                            first: "branch result",
                            duplicate: "call result",
                        } | PlanValidationError::DuplicateStructuredControlProducer {
                            value,
                            first: "call result",
                            duplicate: "branch result",
                        } if *value == ValueRef::new(2)
                    )
                })
        );
    }

    #[test]
    fn rejects_global_structured_producer_conflicts_and_unsourced_bindings() {
        fn has_error(plan: &ExecutionPlan, name: &str) -> bool {
            plan.validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| format!("{error:?}").starts_with(name))
        }

        let mut duplicate_across_branches = valid_plan();
        let original = std::mem::replace(
            &mut duplicate_across_branches.root_region,
            StructuredControlRegion::Sequence(Box::new([])),
        );
        duplicate_across_branches.root_region = StructuredControlRegion::Sequence(Box::new([
            ControlStep::Region(Box::new(original)),
            ControlStep::Region(Box::new(StructuredControlRegion::If {
                condition: ValueRef::new(0),
                then_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
                else_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
                results: Box::new([BranchResultBinding {
                    destination: ValueRef::new(2),
                    then_source: ValueRef::new(3),
                    else_source: ValueRef::new(4),
                    production: Some(OutputProduction::FullyMaterialized),
                }]),
            })),
        ]));
        assert!(has_error(
            &duplicate_across_branches,
            "DuplicateStructuredControlProducer"
        ));

        let mut branch_loop_conflict = valid_plan();
        let StructuredControlRegion::Sequence(steps) = &mut branch_loop_conflict.root_region else {
            unreachable!()
        };
        let ControlStep::Region(loop_region) = &mut steps[2] else {
            unreachable!()
        };
        let StructuredControlRegion::Loop { carried, .. } = loop_region.as_mut() else {
            unreachable!()
        };
        carried[0].result = ValueRef::new(2);
        assert!(has_error(
            &branch_loop_conflict,
            "DuplicateStructuredControlProducer"
        ));

        let mut orphan_declaration = valid_plan();
        orphan_declaration.value_count = 9;
        orphan_declaration.value_sources = orphan_declaration
            .value_sources
            .into_vec()
            .into_iter()
            .chain([PlanValueSource::ControlProduced(
                ValueRef::new(8),
                OutputProduction::FullyMaterialized,
            )])
            .collect::<Vec<_>>()
            .into_boxed_slice();
        assert!(has_error(&orphan_declaration, "OrphanControlProduced"));

        let mut operation_conflict = valid_plan();
        let StructuredControlRegion::Sequence(steps) = &mut operation_conflict.root_region else {
            unreachable!()
        };
        let ControlStep::Region(branch) = &mut steps[1] else {
            unreachable!()
        };
        let StructuredControlRegion::If { results, .. } = branch.as_mut() else {
            unreachable!()
        };
        results[0].destination = ValueRef::new(1);
        for source in &mut operation_conflict.value_sources {
            if *source
                == PlanValueSource::ControlProduced(
                    ValueRef::new(2),
                    OutputProduction::FullyMaterialized,
                )
            {
                *source = PlanValueSource::ControlProduced(
                    ValueRef::new(1),
                    OutputProduction::FullyMaterialized,
                );
            }
        }
        assert!(has_error(
            &operation_conflict,
            "ControlProducedConflictsWithOperationOutput"
        ));

        let mut external_conflict = valid_plan();
        external_conflict.value_count = 9;
        external_conflict.value_sources = external_conflict
            .value_sources
            .into_vec()
            .into_iter()
            .chain([PlanValueSource::ExternalInput(
                ValueRef::new(8),
                OutputProduction::FullyMaterialized,
            )])
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let StructuredControlRegion::Sequence(steps) = &mut external_conflict.root_region else {
            unreachable!()
        };
        let ControlStep::Region(branch) = &mut steps[1] else {
            unreachable!()
        };
        let StructuredControlRegion::If { results, .. } = branch.as_mut() else {
            unreachable!()
        };
        results[0].destination = ValueRef::new(8);
        for source in &mut external_conflict.value_sources {
            if *source
                == PlanValueSource::ControlProduced(
                    ValueRef::new(2),
                    OutputProduction::FullyMaterialized,
                )
            {
                *source = PlanValueSource::ControlProduced(
                    ValueRef::new(8),
                    OutputProduction::FullyMaterialized,
                );
            }
        }
        assert!(has_error(
            &external_conflict,
            "ControlProducedConflictsWithExternalInput"
        ));

        let mut undeclared_destination = valid_plan();
        undeclared_destination.value_sources = undeclared_destination
            .value_sources
            .into_vec()
            .into_iter()
            .filter(|source| {
                *source
                    != PlanValueSource::ControlProduced(
                        ValueRef::new(2),
                        OutputProduction::FullyMaterialized,
                    )
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        assert!(has_error(
            &undeclared_destination,
            "MissingControlProducedDeclaration"
        ));

        let mut unsourced_condition = valid_plan();
        unsourced_condition.value_count = 9;
        let StructuredControlRegion::Sequence(steps) = &mut unsourced_condition.root_region else {
            unreachable!()
        };
        let ControlStep::Region(branch) = &mut steps[1] else {
            unreachable!()
        };
        let StructuredControlRegion::If { condition, .. } = branch.as_mut() else {
            unreachable!()
        };
        *condition = ValueRef::new(8);
        assert!(has_error(
            &unsourced_condition,
            "MissingStructuredBindingSource"
        ));

        let mut unsourced_branch_source = valid_plan();
        unsourced_branch_source.value_count = 9;
        let StructuredControlRegion::Sequence(steps) = &mut unsourced_branch_source.root_region
        else {
            unreachable!()
        };
        let ControlStep::Region(branch) = &mut steps[1] else {
            unreachable!()
        };
        let StructuredControlRegion::If { results, .. } = branch.as_mut() else {
            unreachable!()
        };
        results[0].then_source = ValueRef::new(8);
        assert!(has_error(
            &unsourced_branch_source,
            "MissingStructuredBindingSource"
        ));
    }

    #[test]
    fn rejects_stale_structured_production_facts() {
        let mut branch = valid_plan();
        let StructuredControlRegion::Sequence(steps) = &mut branch.root_region else {
            unreachable!()
        };
        let ControlStep::Region(region) = &mut steps[1] else {
            unreachable!()
        };
        let StructuredControlRegion::If { results, .. } = region.as_mut() else {
            unreachable!()
        };
        results[0].production = Some(OutputProduction::Streaming);
        assert!(
            branch
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| matches!(
                    error,
                    PlanValidationError::StructuredProductionMismatch {
                        producer: "branch result",
                        value,
                        ..
                    } if *value == ValueRef::new(2)
                ))
        );

        let mut missing_branch = valid_plan();
        let StructuredControlRegion::Sequence(steps) = &mut missing_branch.root_region else {
            unreachable!()
        };
        let ControlStep::Region(region) = &mut steps[1] else {
            unreachable!()
        };
        let StructuredControlRegion::If { results, .. } = region.as_mut() else {
            unreachable!()
        };
        results[0].production = None;
        assert!(
            missing_branch
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| matches!(
                    error,
                    PlanValidationError::MissingStructuredProductionFact {
                        producer: "branch result",
                        value,
                    } if *value == ValueRef::new(2)
                ))
        );

        let mut conflicting_branch = valid_plan();
        for source in &mut conflicting_branch.value_sources {
            if source.value() == ValueRef::new(3) {
                *source =
                    PlanValueSource::ExternalInput(ValueRef::new(3), OutputProduction::Streaming);
            }
        }
        assert!(
            conflicting_branch
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| matches!(
                    error,
                    PlanValidationError::ConflictingStructuredProductions {
                        producer: "branch result",
                        value,
                        ..
                    } if *value == ValueRef::new(2)
                ))
        );

        let mut loop_plan = valid_plan();
        let StructuredControlRegion::Sequence(steps) = &mut loop_plan.root_region else {
            unreachable!()
        };
        let ControlStep::Region(region) = &mut steps[2] else {
            unreachable!()
        };
        let StructuredControlRegion::Loop { carried, .. } = region.as_mut() else {
            unreachable!()
        };
        carried[0].production = Some(OutputProduction::Streaming);
        assert!(
            loop_plan
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| matches!(
                    error,
                    PlanValidationError::StructuredProductionMismatch {
                        producer: "loop result",
                        value,
                        ..
                    } if *value == ValueRef::new(6)
                ))
        );

        let mut call = valid_plan();
        let StructuredControlRegion::Sequence(steps) = &mut call.root_region else {
            unreachable!()
        };
        let ControlStep::Region(branch) = &mut steps[1] else {
            unreachable!()
        };
        let StructuredControlRegion::If { else_region, .. } = branch.as_mut() else {
            unreachable!()
        };
        let StructuredControlRegion::Call { results, .. } = else_region.as_mut() else {
            unreachable!()
        };
        results[0].production = Some(OutputProduction::Streaming);
        assert!(call.validate().unwrap_err().0.iter().any(|error| matches!(
            error,
            PlanValidationError::StructuredProductionMismatch {
                producer: "call result",
                value,
                ..
            } if *value == ValueRef::new(7)
        )));
    }

    #[test]
    fn rejects_out_of_bounds_structured_control_bindings() {
        let mut plan = valid_plan();
        let StructuredControlRegion::Sequence(steps) = &mut plan.root_region else {
            unreachable!()
        };
        let ControlStep::Region(branch) = &mut steps[1] else {
            unreachable!()
        };
        let StructuredControlRegion::If { results, .. } = branch.as_mut() else {
            unreachable!()
        };
        results[0].then_source = ValueRef::new(99);
        let ControlStep::Region(loop_region) = &mut steps[2] else {
            unreachable!()
        };
        let StructuredControlRegion::Loop { carried, .. } = loop_region.as_mut() else {
            unreachable!()
        };
        carried[0].next_source = ValueRef::new(98);

        let errors = plan.validate().unwrap_err();
        assert!(errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::ValueOutOfBounds {
                context: "branch then source",
                value,
                ..
            } if *value == ValueRef::new(99)
        )));
        assert!(errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::ValueOutOfBounds {
                context: "loop next source",
                value,
                ..
            } if *value == ValueRef::new(98)
        )));
    }

    #[test]
    fn rejects_out_of_bounds_indices_and_values() {
        let mut plan = valid_plan();
        plan.root_region = StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(
            OperationIndex::new(2),
        )]));
        plan.operations[0].kernel = PlannedKernel::Relational(RelationalSubplanIndex::new(0));
        plan.results[0].value = ValueRef::new(8);

        let errors = plan.validate().unwrap_err();
        assert!(errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::IndexOutOfBounds {
                context: "control step",
                ..
            }
        )));
        assert!(errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::IndexOutOfBounds {
                context: "operation relational subplan",
                ..
            }
        )));
        assert!(errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::ValueOutOfBounds {
                context: "plan result",
                ..
            }
        )));
    }

    #[test]
    fn rejects_value_dependency_self_loops() {
        let mut plan = valid_plan();
        plan.value_dependencies = Box::new([ValueDependency {
            source: ValueRef::new(0),
            destination: ValueRef::new(0),
        }]);

        assert!(plan.validate().unwrap_err().0.iter().any(|error| matches!(
            error,
            PlanValidationError::ValueDependencySelfLoop(value) if *value == ValueRef::new(0)
        )));
    }

    #[test]
    fn rejects_duplicate_value_producers() {
        let mut plan = valid_plan();
        plan.operations[1].outputs[0].value = ValueRef::new(0);

        assert!(plan.validate().unwrap_err().0.iter().any(|error| matches!(
            error,
            PlanValidationError::DuplicateValueProducer { value, .. } if *value == ValueRef::new(0)
        )));
    }

    #[test]
    fn rejects_multi_value_dependency_cycles() {
        let mut plan = valid_plan();
        plan.value_dependencies = Box::new([
            ValueDependency {
                source: ValueRef::new(0),
                destination: ValueRef::new(1),
            },
            ValueDependency {
                source: ValueRef::new(1),
                destination: ValueRef::new(0),
            },
        ]);

        assert!(
            plan.validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| matches!(error, PlanValidationError::ValueDependencyCycle))
        );
    }

    #[test]
    fn rejects_multi_effect_dependency_cycles() {
        let mut plan = valid_plan();
        plan.effect_dependencies = Box::new([
            EffectDependency {
                before: OperationIndex::new(0),
                after: OperationIndex::new(1),
            },
            EffectDependency {
                before: OperationIndex::new(1),
                after: OperationIndex::new(0),
            },
        ]);

        assert!(
            plan.validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| matches!(error, PlanValidationError::EffectDependencyCycle))
        );
    }

    #[test]
    fn rejects_duplicate_relational_subplan_owners() {
        let mut plan = valid_plan();
        plan.relational_subplans = Box::new([RelationalSubplan {
            backend: id("relational.test", RelationalBackendId::new),
            compiled_plan: CompiledRelationalPlan {
                fragment_order: Box::new([id("fragment.test", RelationalFragmentId::new)]),
                operators: Box::new([RelationalOperator::Source {
                    resource: id("database.main", ResourceId::new),
                    relation: "items".into(),
                }]),
                fragment_roots: Box::new([RelationalFragmentRoot {
                    fragment: id("fragment.test", RelationalFragmentId::new),
                    operator: RelationalOperatorIndex::new(0),
                }]),
                roots: Box::new([RelationalOperatorIndex::new(0)]),
                pushdown_hints: Box::new([]),
            },
        }]);
        plan.operations[0].kernel = PlannedKernel::Relational(RelationalSubplanIndex::new(0));
        plan.operations[1].kernel = PlannedKernel::Relational(RelationalSubplanIndex::new(0));

        assert!(plan.validate().unwrap_err().0.iter().any(|error| matches!(
            error,
            PlanValidationError::DuplicateRelationalSubplanOwner {
                subplan,
                first,
                duplicate,
            } if *subplan == RelationalSubplanIndex::new(0)
                && *first == OperationIndex::new(0)
                && *duplicate == OperationIndex::new(1)
        )));
    }

    #[test]
    fn rejects_unowned_relational_subplans() {
        let (mut plan, _) = valid_bridged_plan();
        plan.operations[1].kernel = PlannedKernel::Native(id("kernel.test", KernelHandle::new));

        assert!(plan.validate().unwrap_err().0.iter().any(|error| matches!(
            error,
            PlanValidationError::UnownedRelationalSubplan(subplan)
                if *subplan == RelationalSubplanIndex::new(1)
        )));
    }

    #[test]
    fn rejects_relational_owner_output_root_cardinality_mismatch() {
        let (mut plan, _) = valid_bridged_plan();
        plan.relational_subplans[0].compiled_plan.roots = Box::new([
            RelationalOperatorIndex::new(0),
            RelationalOperatorIndex::new(0),
        ]);

        assert!(plan.validate().unwrap_err().0.iter().any(|error| matches!(
            error,
            PlanValidationError::RelationalOwnerOutputRootCardinalityMismatch {
                subplan,
                owner,
                output_count: 1,
                root_count: 2,
            } if *subplan == RelationalSubplanIndex::new(0)
                && *owner == OperationIndex::new(0)
        )));
    }

    #[test]
    fn rejects_forged_limit_pushdown_over_filter_even_when_rows_match() {
        let (mut plan, _) = valid_bridged_plan();
        plan.relational_subplans[0].compiled_plan.operators = Box::new([
            RelationalOperator::Source {
                resource: id("database.main", ResourceId::new),
                relation: "items".into(),
            },
            RelationalOperator::Filter {
                input: RelationalOperatorIndex::new(0),
                predicate: RelationalExpression::Literal(RelationalLiteral::Boolean(true)),
            },
            RelationalOperator::Limit {
                input: RelationalOperatorIndex::new(1),
                rows: 25,
            },
        ]);
        plan.relational_subplans[0].compiled_plan.pushdown_hints =
            Box::new([RelationalPushdownHint::Limit {
                source: RelationalOperatorIndex::new(0),
                rows: 25,
            }]);

        assert!(plan.validate().is_err());

        let (mut non_source, _) = valid_bridged_plan();
        non_source.relational_subplans[0].compiled_plan.operators = Box::new([
            RelationalOperator::Source {
                resource: id("database.main", ResourceId::new),
                relation: "items".into(),
            },
            RelationalOperator::Filter {
                input: RelationalOperatorIndex::new(0),
                predicate: RelationalExpression::Literal(RelationalLiteral::Boolean(true)),
            },
            RelationalOperator::Limit {
                input: RelationalOperatorIndex::new(1),
                rows: 25,
            },
        ]);
        non_source.relational_subplans[0]
            .compiled_plan
            .pushdown_hints = Box::new([RelationalPushdownHint::Limit {
            source: RelationalOperatorIndex::new(1),
            rows: 25,
        }]);
        assert!(non_source.validate().is_err());

        let (mut mismatched_rows, _) = valid_bridged_plan();
        mismatched_rows.relational_subplans[0]
            .compiled_plan
            .operators = Box::new([
            RelationalOperator::Source {
                resource: id("database.main", ResourceId::new),
                relation: "items".into(),
            },
            RelationalOperator::Limit {
                input: RelationalOperatorIndex::new(0),
                rows: 25,
            },
        ]);
        mismatched_rows.relational_subplans[0]
            .compiled_plan
            .pushdown_hints = Box::new([RelationalPushdownHint::Limit {
            source: RelationalOperatorIndex::new(0),
            rows: 24,
        }]);
        assert!(mismatched_rows.validate().is_err());
    }

    #[test]
    fn rejects_projection_pushdown_that_does_not_exactly_match_direct_source_projection() {
        let (mut unsafe_projection, _) = valid_bridged_plan();
        unsafe_projection.relational_subplans[0]
            .compiled_plan
            .operators = Box::new([
            RelationalOperator::Source {
                resource: id("database.main", ResourceId::new),
                relation: "items".into(),
            },
            RelationalOperator::Project {
                input: RelationalOperatorIndex::new(0),
                columns: Box::new([RelationalProjection {
                    name: "constant".into(),
                    expression: RelationalExpression::Literal(RelationalLiteral::Integer(1)),
                }]),
            },
        ]);
        unsafe_projection.relational_subplans[0]
            .compiled_plan
            .pushdown_hints = Box::new([RelationalPushdownHint::Projection {
            source: RelationalOperatorIndex::new(0),
            columns: Box::new(["constant".into()]),
        }]);
        assert!(unsafe_projection.validate().is_err());

        let (mut mismatched_columns, _) = valid_bridged_plan();
        mismatched_columns.relational_subplans[0]
            .compiled_plan
            .operators = Box::new([
            RelationalOperator::Source {
                resource: id("database.main", ResourceId::new),
                relation: "items".into(),
            },
            RelationalOperator::Project {
                input: RelationalOperatorIndex::new(0),
                columns: Box::new([RelationalProjection {
                    name: "renamed".into(),
                    expression: RelationalExpression::Column("actual".into()),
                }]),
            },
        ]);
        mismatched_columns.relational_subplans[0]
            .compiled_plan
            .pushdown_hints = Box::new([RelationalPushdownHint::Projection {
            source: RelationalOperatorIndex::new(0),
            columns: Box::new(["forged".into()]),
        }]);
        assert!(mismatched_columns.validate().is_err());
    }

    #[test]
    fn rejects_forged_or_stale_predicate_lineage_hints_purely() {
        let (mut plan, _) = valid_bridged_plan();

        let predicate = RelationalExpression::Equal(
            Box::new(RelationalExpression::Column("status".into())),
            Box::new(RelationalExpression::Literal(RelationalLiteral::String(
                "paid".into(),
            ))),
        );
        let compiled = &mut plan.relational_subplans[0].compiled_plan;
        compiled.operators = Box::new([
            RelationalOperator::Source {
                resource: id("database.main", ResourceId::new),
                relation: "items".into(),
            },
            RelationalOperator::Filter {
                input: RelationalOperatorIndex::new(0),
                predicate: predicate.clone(),
            },
            RelationalOperator::Project {
                input: RelationalOperatorIndex::new(1),
                columns: Box::new([RelationalProjection {
                    name: "amount".into(),
                    expression: RelationalExpression::Column("amount".into()),
                }]),
            },
        ]);
        compiled.fragment_roots[0].operator = RelationalOperatorIndex::new(2);
        compiled.roots = Box::new([RelationalOperatorIndex::new(2)]);
        compiled.pushdown_hints = Box::new([
            RelationalPushdownHint::Projection {
                source: RelationalOperatorIndex::new(0),
                columns: Box::new(["amount".into(), "status".into()]),
            },
            RelationalPushdownHint::Predicate {
                source: RelationalOperatorIndex::new(0),
                predicate: predicate.clone(),
            },
        ]);
        plan.validate().expect("exact inferred hints validate");

        let semantic_operators = plan.relational_subplans[0].compiled_plan.operators.clone();
        let semantic_roots = plan.relational_subplans[0].compiled_plan.roots.clone();
        let mut removed = plan.clone();
        removed.relational_subplans[0].compiled_plan.pushdown_hints = Box::new([]);
        assert_eq!(
            removed.relational_subplans[0].compiled_plan.operators,
            semantic_operators
        );
        assert_eq!(
            removed.relational_subplans[0].compiled_plan.roots,
            semantic_roots
        );
        assert!(
            removed
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| matches!(
                    error,
                    PlanValidationError::RelationalPushdownHintsMismatch { subplan }
                        if *subplan == RelationalSubplanIndex::new(0)
                ))
        );

        let mut forged = plan;
        forged.relational_subplans[0].compiled_plan.pushdown_hints[1] =
            RelationalPushdownHint::Predicate {
                source: RelationalOperatorIndex::new(0),
                predicate: RelationalExpression::Equal(
                    Box::new(RelationalExpression::Column("forged".into())),
                    Box::new(RelationalExpression::Literal(RelationalLiteral::String(
                        "paid".into(),
                    ))),
                ),
            };
        assert!(
            forged
                .validate()
                .unwrap_err()
                .0
                .iter()
                .any(|error| matches!(
                    error,
                    PlanValidationError::RelationalPushdownHintsMismatch { subplan }
                        if *subplan == RelationalSubplanIndex::new(0)
                ))
        );
    }

    #[test]
    fn accepts_declared_external_and_control_produced_input_sources() {
        let mut external = valid_plan();
        external.value_count = 9;
        external
            .value_contracts
            .insert(ValueRef::new(8), PlannedValueContract::opaque());
        external.value_sources = external
            .value_sources
            .into_vec()
            .into_iter()
            .chain([PlanValueSource::ExternalInput(
                ValueRef::new(8),
                OutputProduction::FullyMaterialized,
            )])
            .collect::<Vec<_>>()
            .into_boxed_slice();
        external.operations[1].inputs = Box::new([PlannedInput {
            value: ValueRef::new(8),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            consumption: crate::node_system::protocol::InputConsumption::FullyMaterialized,
            bound_value: None,
        }]);
        external.value_dependencies = Box::new([]);
        assert_eq!(external.validate(), Ok(()));

        let mut control_produced = valid_plan();
        control_produced.operations[1].inputs = Box::new([PlannedInput {
            value: ValueRef::new(5),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            consumption: crate::node_system::protocol::InputConsumption::FullyMaterialized,
            bound_value: None,
        }]);
        control_produced.value_dependencies = Box::new([]);
        assert_eq!(control_produced.validate(), Ok(()));
    }

    #[test]
    fn bound_input_operation_propagates_outputs_to_downstream_and_result_validation() {
        let mut first = operation(1);
        first.inputs = Box::new([PlannedInput {
            value: ValueRef::new(0),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            consumption: InputConsumption::FullyMaterialized,
            bound_value: Some(Value::Integer(7)),
        }]);
        let mut second = operation(2);
        second.inputs = Box::new([PlannedInput {
            value: ValueRef::new(1),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            consumption: InputConsumption::FullyMaterialized,
            bound_value: None,
        }]);
        let mut plan = valid_plan();
        plan.value_count = 3;
        plan.value_contracts.retain(|value, _| value.index() < 3);
        plan.operations = Box::new([first, second]);
        plan.value_sources = Box::new([]);
        plan.value_dependencies = Box::new([]);
        plan.root_region = StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ]));
        plan.effect_dependencies = Box::new([]);
        plan.resources = Box::new([]);
        plan.results[0].value = ValueRef::new(2);
        plan.publications[0] = PlannedPublication::GraphResult {
            name: plan.results[0].name.clone(),
            output: plan.results[0].output.clone(),
            value: ValueRef::new(2),
        };

        assert_eq!(plan.validate(), Ok(()));
    }

    #[test]
    fn conflicting_alias_productions_are_typed_and_permutation_stable() {
        let build = |reverse: bool| {
            let mut plan = valid_plan();
            plan.value_count = 9;
            for source in &mut plan.value_sources {
                if source.value() == ValueRef::new(3) {
                    *source = PlanValueSource::ExternalInput(
                        ValueRef::new(3),
                        OutputProduction::Streaming,
                    );
                }
            }
            let mut aliases = vec![
                ValueDependency {
                    source: ValueRef::new(0),
                    destination: ValueRef::new(8),
                },
                ValueDependency {
                    source: ValueRef::new(3),
                    destination: ValueRef::new(8),
                },
            ];
            if reverse {
                aliases.reverse();
            }
            plan.value_dependencies = aliases.into_boxed_slice();
            plan
        };

        let errors = [build(false), build(true)].map(|plan| {
            plan.validate()
                .expect_err("multiple conflicting aliases must be rejected")
                .0
                .into_vec()
                .into_iter()
                .filter(|error| {
                    matches!(
                        error,
                        PlanValidationError::DuplicateValueDependencyAlias { destination, .. }
                            | PlanValidationError::ConflictingAliasProductions {
                                destination,
                                ..
                            } if *destination == ValueRef::new(8)
                    )
                })
                .collect::<Vec<_>>()
        });

        assert_eq!(errors[0], errors[1]);
        assert!(errors[0].iter().any(|error| matches!(
            error,
            PlanValidationError::DuplicateValueDependencyAlias { sources, .. }
                if sources.as_ref() == [ValueRef::new(0), ValueRef::new(3)]
        )));
        assert!(errors[0].iter().any(|error| matches!(
            error,
            PlanValidationError::ConflictingAliasProductions { productions, .. }
                if productions.as_ref()
                    == [OutputProduction::Streaming, OutputProduction::FullyMaterialized]
        )));
    }

    #[test]
    fn duplicate_aliases_propagate_matching_production_but_still_fail_typed_validation() {
        let mut plan = valid_plan();
        plan.value_count = 9;
        plan.value_dependencies = Box::new([
            ValueDependency {
                source: ValueRef::new(0),
                destination: ValueRef::new(8),
            },
            ValueDependency {
                source: ValueRef::new(3),
                destination: ValueRef::new(8),
            },
        ]);
        let StructuredControlRegion::Sequence(steps) = &mut plan.root_region else {
            unreachable!()
        };
        let ControlStep::Region(branch) = &mut steps[1] else {
            unreachable!()
        };
        let StructuredControlRegion::If { results, .. } = branch.as_mut() else {
            unreachable!()
        };
        results[0].then_source = ValueRef::new(8);

        let errors = plan
            .validate()
            .expect_err("duplicate aliases must be rejected even when contracts agree");
        assert!(errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::DuplicateValueDependencyAlias { destination, sources }
                if *destination == ValueRef::new(8)
                    && sources.as_ref() == [ValueRef::new(0), ValueRef::new(3)]
        )));
        assert!(!errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::MissingStructuredProductionFact {
                producer: "branch result",
                ..
            }
        )));
    }

    fn concrete_type(id: &str) -> TypeExpr {
        TypeExpr::Concrete(TypeId::new(id).expect("test type ID is valid"))
    }

    fn data_series_contract(element_type: TypeExpr) -> PlannedValueContract {
        PlannedValueContract {
            kind: PlannedValueKind::DataSeries,
            type_expr: data_series_type(element_type),
        }
    }

    fn set_dependency_contracts(
        plan: &mut ExecutionPlan,
        source: PlannedValueContract,
        destination: PlannedValueContract,
    ) {
        plan.value_contracts
            .insert(ValueRef::new(0), source.clone());
        plan.value_contracts
            .insert(ValueRef::new(1), destination.clone());
        plan.operations[0].outputs[0].contract = source;
        plan.operations[1].outputs[0].contract = destination;
    }

    #[test]
    fn accepts_value_dependency_assignable_to_destination_union() {
        let mut plan = valid_plan();
        set_dependency_contracts(
            &mut plan,
            data_series_contract(concrete_type("core.float64")),
            data_series_contract(TypeExpr::Union(vec![
                concrete_type("core.int64"),
                concrete_type("core.float64"),
            ])),
        );

        plan.validate()
            .expect("a concrete numeric series must be assignable to the numeric series union");
    }

    #[test]
    fn rejects_value_dependency_outside_destination_union() {
        let mut plan = valid_plan();
        set_dependency_contracts(
            &mut plan,
            data_series_contract(concrete_type("core.string")),
            data_series_contract(TypeExpr::Union(vec![
                concrete_type("core.int64"),
                concrete_type("core.float64"),
            ])),
        );

        assert!(plan.validate().unwrap_err().0.iter().any(|error| matches!(
            error,
            PlanValidationError::ValueContractMismatch {
                context: "value dependency",
                source,
                destination,
                ..
            } if *source == ValueRef::new(0) && *destination == ValueRef::new(1)
        )));
    }

    #[test]
    fn rejects_undeclared_dependency_root_as_input_source() {
        let mut plan = valid_plan();
        plan.value_count = 10;
        plan.operations[1].inputs = Box::new([PlannedInput {
            value: ValueRef::new(9),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            consumption: crate::node_system::protocol::InputConsumption::FullyMaterialized,
            bound_value: None,
        }]);
        plan.value_dependencies = Box::new([ValueDependency {
            source: ValueRef::new(8),
            destination: ValueRef::new(9),
        }]);

        assert!(plan.validate().unwrap_err().0.iter().any(|error| matches!(
            error,
            PlanValidationError::MissingInputSource { value, operation }
                if *value == ValueRef::new(9) && *operation == OperationIndex::new(1)
        )));
    }
}
