//! Pure, immutable execution-plan contracts.
//!
//! This module deliberately contains no registry lookup, graph document access,
//! I/O, acquired resources, or run state. All compact indices are local to one
//! `ExecutionPlan` and are intentionally not serializable.

mod model;
mod validation;

pub use model::*;
pub use validation::{PlanValidationError, PlanValidationErrors};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::{
        CompilationBasis, CompileId, CompileProvenance, ProjectSessionId, ResourceVersionSet,
    };
    use crate::node_system::document::{GraphResourcePath, GraphRevision, NodeId};
    use crate::node_system::protocol::{NodeTypeId, OutputProduction};
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
                },
                compile_id: CompileId::new(1),
            },
            value_count: 2,
            operations: Box::new([operation(0), operation(1)]),
            value_sources: Box::new([]),
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
                        arguments: Box::new([RegionValueBinding {
                            destination: ValueRef::new(1),
                            source: ValueRef::new(0),
                        }]),
                        results: Box::new([]),
                    }),
                    results: Box::new([]),
                })),
                ControlStep::Region(Box::new(StructuredControlRegion::Loop {
                    body: Box::new(StructuredControlRegion::Sequence(Box::new([
                        ControlStep::Operation(OperationIndex::new(1)),
                    ]))),
                    carried: Box::new([LoopCarriedBinding {
                        body_input: ValueRef::new(1),
                        initial_source: ValueRef::new(0),
                        next_source: ValueRef::new(1),
                        result: ValueRef::new(1),
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
                value: ValueRef::new(1),
            }]),
        }
    }

    #[test]
    fn validates_all_structured_region_variants() {
        assert_eq!(valid_plan().validate(), Ok(()));
    }

    #[test]
    fn rejects_out_of_bounds_indices_and_values() {
        let mut plan = valid_plan();
        plan.root_region = StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(
            OperationIndex::new(2),
        )]));
        plan.operations[0].kernel = PlannedKernel::Relational(RelationalSubplanIndex::new(0));
        plan.results[0].value = ValueRef::new(2);

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
    fn accepts_declared_external_and_control_produced_input_sources() {
        for source in [
            PlanValueSource::ExternalInput(ValueRef::new(2)),
            PlanValueSource::ControlProduced(ValueRef::new(2)),
        ] {
            let mut plan = valid_plan();
            plan.value_count = 3;
            plan.value_sources = Box::new([source]);
            plan.operations[1].inputs = Box::new([PlannedInput {
                value: ValueRef::new(2),
                consumption: crate::node_system::protocol::InputConsumption::FullyMaterialized,
            }]);
            plan.value_dependencies = Box::new([]);

            assert_eq!(plan.validate(), Ok(()));
        }
    }

    #[test]
    fn rejects_undeclared_dependency_root_as_input_source() {
        let mut plan = valid_plan();
        plan.value_count = 4;
        plan.operations[1].inputs = Box::new([PlannedInput {
            value: ValueRef::new(3),
            consumption: crate::node_system::protocol::InputConsumption::FullyMaterialized,
        }]);
        plan.value_dependencies = Box::new([ValueDependency {
            source: ValueRef::new(2),
            destination: ValueRef::new(3),
        }]);

        assert!(plan.validate().unwrap_err().0.iter().any(|error| matches!(
            error,
            PlanValidationError::MissingInputSource { value, operation }
                if *value == ValueRef::new(3) && *operation == OperationIndex::new(1)
        )));
    }
}
