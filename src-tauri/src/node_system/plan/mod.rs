//! Pure, immutable execution-plan contracts.
//!
//! This module deliberately contains no registry lookup, graph document access,
//! I/O, acquired resources, or run state. All compact indices are local to one
//! `ExecutionPlan` and serialize only as part of that immutable plan product.

mod model;
mod validation;

pub use model::*;
pub(crate) use validation::PlanSourceFacts;
pub use validation::{PlanValidationError, PlanValidationErrors};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::{
        CompilationBasis, CompileId, CompileProvenance, ProjectSessionId, ResourceVersionSet,
    };
    use crate::node_system::document::{GraphResourcePath, GraphRevision, NodeId, PortAddress};
    use crate::node_system::protocol::{
        InputConsumption, NodeTypeId, OutputProduction, PortKey, Value,
    };
    use crate::node_system::registry::RegistryFingerprint;

    fn id<T>(value: &str, constructor: impl FnOnce(Box<str>) -> Result<T, InvalidPlanId>) -> T {
        constructor(value.into()).unwrap()
    }

    fn operation(output: u32) -> PlannedOperation {
        PlannedOperation {
            source_node_id: NodeId::from_uuid(uuid::Uuid::nil()),
            source_node_type_id: NodeTypeId::new("yssbi.test.node").unwrap(),
            kernel: PlannedKernel::Native(id("kernel.test", KernelHandle::new)),
            inputs: Box::new([]),
            outputs: Box::new([PlannedOutput {
                value: ValueRef::new(output),
                production: OutputProduction::FullyMaterialized,
            }]),
            params: id("params-1", CompiledParameterHandle::new),
        }
    }

    fn valid_plan() -> ExecutionPlan {
        ExecutionPlan {
            provenance: CompileProvenance {
                project_session_id: ProjectSessionId::new("test-session"),
                graph_path: GraphResourcePath("events/test".into()),
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
            value_sources: Box::new([
                PlanValueSource::ExternalInput(ValueRef::new(3)),
                PlanValueSource::ExternalInput(ValueRef::new(4)),
                PlanValueSource::ControlProduced(ValueRef::new(2)),
                PlanValueSource::ControlProduced(ValueRef::new(5)),
                PlanValueSource::ControlProduced(ValueRef::new(6)),
                PlanValueSource::ControlProduced(ValueRef::new(7)),
            ]),
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
                        }]),
                        mandatory: true,
                    }),
                    results: Box::new([BranchResultBinding {
                        destination: ValueRef::new(2),
                        then_source: ValueRef::new(3),
                        else_source: ValueRef::new(4),
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
                output: GraphOutputRef {
                    graph_path: GraphResourcePath("events/test".into()),
                    port: PortAddress::declared(
                        NodeId::from_uuid(uuid::Uuid::nil()),
                        PortKey::new("result").unwrap(),
                    ),
                },
                value: ValueRef::new(6),
            }]),
        }
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
        assert!(matches!(
            duplicate_output.validate().unwrap_err().0.as_ref(),
            [PlanValidationError::DuplicateResultOutput(_)]
        ));

        let mut duplicate_name = valid_plan();
        duplicate_name.results = Box::new([
            duplicate_name.results[0].clone(),
            PlanResult {
                name: "result".into(),
                output: GraphOutputRef {
                    graph_path: GraphResourcePath("events/test".into()),
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

    fn valid_bridged_plan() -> (ExecutionPlan, PlannedMaterializationBridge) {
        let mut plan = valid_plan();
        let producer = id("fragment.producer", RelationalFragmentId::new);
        let consumer = id("fragment.consumer", RelationalFragmentId::new);
        let bridge = PlannedMaterializationBridge {
            producer_fragment: producer.clone(),
            consumer_fragment: consumer.clone(),
            producer_subplan: RelationalSubplanIndex::new(0),
            consumer_subplan: RelationalSubplanIndex::new(1),
            bridge: MaterializationBridge::Collect,
        };
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
                        fragment: producer.clone(),
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    bridge_inputs: Box::new([]),
                    requested_fragment_outputs: Box::new([producer]),
                    roots: Box::new([RelationalOperatorIndex::new(0)]),
                    pushdown_hints: Box::new([]),
                },
                materialization_bridges: Box::new([]),
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
                    bridge_inputs: Box::new([RelationalBridgeInput {
                        operator: RelationalOperatorIndex::new(0),
                        bridge: bridge.clone(),
                    }]),
                    requested_fragment_outputs: Box::new([]),
                    roots: Box::new([RelationalOperatorIndex::new(0)]),
                    pushdown_hints: Box::new([]),
                },
                materialization_bridges: Box::new([bridge.clone()]),
            },
        ]);
        plan.operations[0].kernel = PlannedKernel::Relational(RelationalSubplanIndex::new(0));
        plan.operations[1].kernel = PlannedKernel::Relational(RelationalSubplanIndex::new(1));
        (plan, bridge)
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
            .filter(|source| *source != PlanValueSource::ControlProduced(ValueRef::new(7)))
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
            .chain([PlanValueSource::ControlProduced(ValueRef::new(8))])
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
            if *source == PlanValueSource::ControlProduced(ValueRef::new(2)) {
                *source = PlanValueSource::ControlProduced(ValueRef::new(1));
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
            .chain([PlanValueSource::ExternalInput(ValueRef::new(8))])
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
            if *source == PlanValueSource::ControlProduced(ValueRef::new(2)) {
                *source = PlanValueSource::ControlProduced(ValueRef::new(8));
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
            .filter(|source| *source != PlanValueSource::ControlProduced(ValueRef::new(2)))
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
                bridge_inputs: Box::new([]),
                requested_fragment_outputs: Box::new([]),
                roots: Box::new([RelationalOperatorIndex::new(0)]),
                pushdown_hints: Box::new([]),
            },
            materialization_bridges: Box::new([]),
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
    fn rejects_invalid_requested_relational_fragment_outputs() {
        let mut plan = valid_plan();
        let fragment = id("fragment.test", RelationalFragmentId::new);
        let missing = id("fragment.missing", RelationalFragmentId::new);
        plan.relational_subplans = Box::new([RelationalSubplan {
            backend: id("relational.test", RelationalBackendId::new),
            compiled_plan: CompiledRelationalPlan {
                fragment_order: Box::new([fragment.clone()]),
                operators: Box::new([RelationalOperator::Source {
                    resource: id("database.main", ResourceId::new),
                    relation: "items".into(),
                }]),
                fragment_roots: Box::new([RelationalFragmentRoot {
                    fragment: fragment.clone(),
                    operator: RelationalOperatorIndex::new(0),
                }]),
                bridge_inputs: Box::new([]),
                requested_fragment_outputs: Box::new([
                    fragment.clone(),
                    fragment.clone(),
                    missing.clone(),
                ]),
                roots: Box::new([RelationalOperatorIndex::new(0)]),
                pushdown_hints: Box::new([]),
            },
            materialization_bridges: Box::new([]),
        }]);
        plan.operations[0].kernel = PlannedKernel::Relational(RelationalSubplanIndex::new(0));

        let errors = plan.validate().unwrap_err();

        assert!(errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::RelationalFragmentOutputDuplicate(id) if id == &fragment
        )));
        assert!(errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::RelationalFragmentOutputUnexpected(id) if id == &missing
        )));
    }

    #[test]
    fn rejects_bridge_whose_producer_output_is_not_requested() {
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
                        fragment: producer.clone(),
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    bridge_inputs: Box::new([]),
                    requested_fragment_outputs: Box::new([]),
                    roots: Box::new([RelationalOperatorIndex::new(0)]),
                    pushdown_hints: Box::new([]),
                },
                materialization_bridges: Box::new([]),
            },
            RelationalSubplan {
                backend: id("relational.test", RelationalBackendId::new),
                compiled_plan: CompiledRelationalPlan {
                    fragment_order: Box::new([consumer.clone()]),
                    operators: Box::new([RelationalOperator::Input {
                        name: "input".into(),
                    }]),
                    fragment_roots: Box::new([RelationalFragmentRoot {
                        fragment: consumer.clone(),
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    bridge_inputs: Box::new([]),
                    requested_fragment_outputs: Box::new([]),
                    roots: Box::new([RelationalOperatorIndex::new(0)]),
                    pushdown_hints: Box::new([]),
                },
                materialization_bridges: Box::new([PlannedMaterializationBridge {
                    producer_fragment: producer.clone(),
                    consumer_fragment: consumer,
                    producer_subplan: RelationalSubplanIndex::new(0),
                    consumer_subplan: RelationalSubplanIndex::new(1),
                    bridge: MaterializationBridge::Collect,
                }]),
            },
        ]);
        plan.operations[0].kernel = PlannedKernel::Relational(RelationalSubplanIndex::new(0));
        plan.operations[1].kernel = PlannedKernel::Relational(RelationalSubplanIndex::new(1));

        let errors = plan.validate().unwrap_err();

        assert!(errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::BridgeProducerOutputNotRequested {
                producer_subplan,
                fragment,
            } if *producer_subplan == RelationalSubplanIndex::new(0) && fragment == &producer
        )));
    }

    #[test]
    fn rejects_invalid_relational_bridge_input_operators() {
        let (mut out_of_bounds, _) = valid_bridged_plan();
        out_of_bounds.relational_subplans[1]
            .compiled_plan
            .bridge_inputs[0]
            .operator = RelationalOperatorIndex::new(1);
        assert!(out_of_bounds.validate().unwrap_err().0.iter().any(|error| {
            matches!(
                error,
                PlanValidationError::RelationalBridgeInputOperatorOutOfBounds {
                    subplan,
                    operator,
                    operator_count: 1,
                } if *subplan == RelationalSubplanIndex::new(1)
                    && *operator == RelationalOperatorIndex::new(1)
            )
        }));

        let (mut not_input, _) = valid_bridged_plan();
        not_input.relational_subplans[1].compiled_plan.operators[0] = RelationalOperator::Source {
            resource: id("database.main", ResourceId::new),
            relation: "items".into(),
        };
        assert!(not_input.validate().unwrap_err().0.iter().any(|error| {
            matches!(
                error,
                PlanValidationError::RelationalBridgeInputOperatorNotInput {
                    subplan,
                    operator,
                } if *subplan == RelationalSubplanIndex::new(1)
                    && *operator == RelationalOperatorIndex::new(0)
            )
        }));
    }

    #[test]
    fn rejects_duplicate_relational_bridge_input_bindings() {
        let (mut plan, bridge) = valid_bridged_plan();
        plan.relational_subplans[1].compiled_plan.operators = Box::new([
            RelationalOperator::Input { name: "a".into() },
            RelationalOperator::Input { name: "b".into() },
        ]);
        plan.relational_subplans[1].compiled_plan.bridge_inputs = Box::new([
            RelationalBridgeInput {
                operator: RelationalOperatorIndex::new(0),
                bridge: bridge.clone(),
            },
            RelationalBridgeInput {
                operator: RelationalOperatorIndex::new(0),
                bridge: bridge.clone(),
            },
            RelationalBridgeInput {
                operator: RelationalOperatorIndex::new(1),
                bridge,
            },
        ]);

        let errors = plan.validate().unwrap_err();
        assert!(errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::DuplicateRelationalBridgeInputOperator {
                subplan,
                operator,
            } if *subplan == RelationalSubplanIndex::new(1)
                && *operator == RelationalOperatorIndex::new(0)
        )));
        assert!(errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::DuplicateRelationalBridgeInputBridge {
                subplan,
                bridge: duplicate,
            } if *subplan == RelationalSubplanIndex::new(1) && duplicate == &plan.relational_subplans[1].materialization_bridges[0]
        )));
    }

    #[test]
    fn rejects_inconsistent_relational_bridge_subplan_and_fragment_identities() {
        let (mut wrong_producer, _) = valid_bridged_plan();
        let missing_producer = id("fragment.missing-producer", RelationalFragmentId::new);
        wrong_producer.relational_subplans[1].materialization_bridges[0].producer_fragment =
            missing_producer.clone();
        wrong_producer.relational_subplans[1]
            .compiled_plan
            .bridge_inputs[0]
            .bridge
            .producer_fragment = missing_producer.clone();
        assert!(wrong_producer.validate().unwrap_err().0.iter().any(|error| {
            matches!(error, PlanValidationError::BridgeFragmentMissing(fragment) if fragment == &missing_producer)
        }));

        let (mut wrong_consumer, _) = valid_bridged_plan();
        let missing_consumer = id("fragment.missing-consumer", RelationalFragmentId::new);
        wrong_consumer.relational_subplans[1].materialization_bridges[0].consumer_fragment =
            missing_consumer.clone();
        wrong_consumer.relational_subplans[1]
            .compiled_plan
            .bridge_inputs[0]
            .bridge
            .consumer_fragment = missing_consumer.clone();
        assert!(wrong_consumer.validate().unwrap_err().0.iter().any(|error| {
            matches!(error, PlanValidationError::BridgeFragmentMissing(fragment) if fragment == &missing_consumer)
        }));

        let (mut wrong_subplan, _) = valid_bridged_plan();
        wrong_subplan.relational_subplans[1].materialization_bridges[0].consumer_subplan =
            RelationalSubplanIndex::new(0);
        wrong_subplan.relational_subplans[1]
            .compiled_plan
            .bridge_inputs[0]
            .bridge
            .consumer_subplan = RelationalSubplanIndex::new(0);
        assert!(wrong_subplan.validate().unwrap_err().0.iter().any(|error| {
            matches!(
                error,
                PlanValidationError::BridgeStoredOnWrongConsumer {
                    stored_on,
                    consumer,
                } if *stored_on == RelationalSubplanIndex::new(1)
                    && *consumer == RelationalSubplanIndex::new(0)
            )
        }));
    }

    #[test]
    fn rejects_missing_or_inconsistent_relational_bridge_input_identity() {
        let (mut plan, declared) = valid_bridged_plan();
        plan.relational_subplans[1].compiled_plan.bridge_inputs[0]
            .bridge
            .producer_fragment = id("fragment.other", RelationalFragmentId::new);

        let errors = plan.validate().unwrap_err();
        assert!(errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::RelationalBridgeInputBridgeUndeclared {
                subplan,
                operator,
                ..
            } if *subplan == RelationalSubplanIndex::new(1)
                && *operator == RelationalOperatorIndex::new(0)
        )));
        assert!(errors.0.iter().any(|error| matches!(
            error,
            PlanValidationError::RelationalBridgeInputMissing {
                subplan,
                bridge,
            } if *subplan == RelationalSubplanIndex::new(1) && bridge == &declared
        )));
    }

    #[test]
    fn accepts_declared_external_and_control_produced_input_sources() {
        let mut external = valid_plan();
        external.value_count = 9;
        external.value_sources = external
            .value_sources
            .into_vec()
            .into_iter()
            .chain([PlanValueSource::ExternalInput(ValueRef::new(8))])
            .collect::<Vec<_>>()
            .into_boxed_slice();
        external.operations[1].inputs = Box::new([PlannedInput {
            value: ValueRef::new(8),
            consumption: crate::node_system::protocol::InputConsumption::FullyMaterialized,
            bound_value: None,
        }]);
        external.value_dependencies = Box::new([]);
        assert_eq!(external.validate(), Ok(()));

        let mut control_produced = valid_plan();
        control_produced.operations[1].inputs = Box::new([PlannedInput {
            value: ValueRef::new(5),
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
            consumption: InputConsumption::FullyMaterialized,
            bound_value: Some(Value::Integer(7)),
        }]);
        let mut second = operation(2);
        second.inputs = Box::new([PlannedInput {
            value: ValueRef::new(1),
            consumption: InputConsumption::FullyMaterialized,
            bound_value: None,
        }]);
        let mut plan = valid_plan();
        plan.value_count = 3;
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

        assert_eq!(plan.validate(), Ok(()));
    }

    #[test]
    fn rejects_undeclared_dependency_root_as_input_source() {
        let mut plan = valid_plan();
        plan.value_count = 10;
        plan.operations[1].inputs = Box::new([PlannedInput {
            value: ValueRef::new(9),
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
