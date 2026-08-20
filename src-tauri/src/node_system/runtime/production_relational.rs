use super::relational_dataframe::{
    filter_dataframe, project_dataframe, rename_dataframe, tabular_runtime_to_dataframe,
};
use super::{
    CancellationToken, KernelError, ProjectResourceLease, RelationalBackend, RelationalContext,
    RelationalError, RelationalErrorCode, RelationalExecution, RuntimeValue,
    dataframe_to_protocol_value_with_checkpoint,
};
use crate::node_system::plan::{
    CompiledRelationalPlan, RelationalOperator, RelationalOperatorIndex,
    infer_relational_pushdown_hints,
};
use polars::prelude::DataFrame;
use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductionRelationalCheckpoint {
    OperatorEvaluation,
    SourceScan,
    PredicateEvaluation,
    ResultMaterialization,
    ResultConversion,
}

#[cfg(test)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProductionRelationalObservation {
    pub relational_islands: Option<usize>,
    pub backend_invocations: usize,
    pub scan_limits: Vec<Option<usize>>,
    pub relational_subplans: Vec<crate::node_system::plan::RelationalSubplan>,
    pub relational_result_bindings:
        Vec<(Box<str>, crate::node_system::plan::RelationalOperatorIndex)>,
}

#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct ProductionRelationalObserver {
    observation: std::sync::Mutex<ProductionRelationalObservation>,

    materialized_dataframes: std::sync::Mutex<Vec<DataFrame>>,
}

#[cfg(test)]
impl ProductionRelationalObserver {
    pub(crate) fn observe_plan(&self, plan: &crate::node_system::plan::ExecutionPlan) {
        let mut observation = self.observation.lock().unwrap();
        observation.relational_islands = Some(plan.relational_subplans.len());
        observation.relational_subplans = plan.relational_subplans.to_vec();
        observation.relational_result_bindings.clear();
        for operation in &plan.operations {
            let crate::node_system::plan::PlannedKernel::Relational(subplan_index) =
                &operation.kernel
            else {
                continue;
            };
            let Some(subplan) = plan.relational_subplans.get(subplan_index.index()) else {
                continue;
            };
            for (output, root) in operation.outputs.iter().zip(&subplan.compiled_plan.roots) {
                if let Some(result) = plan
                    .results
                    .iter()
                    .find(|result| result.value == output.value)
                {
                    observation
                        .relational_result_bindings
                        .push((result.name.clone(), *root));
                }
            }
        }
    }

    fn observe_invocation(&self) {
        self.observation.lock().unwrap().backend_invocations += 1;
    }

    fn observe_scan(&self, limit: Option<usize>) {
        self.observation.lock().unwrap().scan_limits.push(limit);
    }

    fn observe_materialization(&self, dataframe: &DataFrame) {
        self.materialized_dataframes
            .lock()
            .unwrap()
            .push(dataframe.clone());
    }

    pub(crate) fn materialized_dataframes(&self) -> Vec<DataFrame> {
        self.materialized_dataframes.lock().unwrap().clone()
    }

    pub(crate) fn snapshot(&self) -> ProductionRelationalObservation {
        self.observation.lock().unwrap().clone()
    }
}

#[derive(Default)]
pub struct ProductionRelationalBackend {
    #[cfg(test)]
    scan_limits: Option<Arc<std::sync::Mutex<Vec<Option<usize>>>>>,
    #[cfg(test)]
    observer: Option<Arc<ProductionRelationalObserver>>,
    #[cfg(test)]
    checkpoint: Option<
        Arc<dyn Fn(ProductionRelationalCheckpoint, &CancellationToken) + Send + Sync + 'static>,
    >,
}

impl ProductionRelationalBackend {
    #[cfg(test)]
    pub(crate) fn recording_scan_limits(
        observed: Arc<std::sync::Mutex<Vec<Option<usize>>>>,
    ) -> Self {
        Self {
            scan_limits: Some(observed),
            observer: None,
            checkpoint: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_observer(observer: Arc<ProductionRelationalObserver>) -> Self {
        Self {
            scan_limits: None,
            observer: Some(observer),
            checkpoint: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_test_checkpoint(
        mut self,
        checkpoint: Arc<
            dyn Fn(ProductionRelationalCheckpoint, &CancellationToken) + Send + Sync + 'static,
        >,
    ) -> Self {
        self.checkpoint = Some(checkpoint);
        self
    }

    fn checkpoint(
        &self,
        #[cfg(test)] checkpoint: ProductionRelationalCheckpoint,
        cancellation: &CancellationToken,
    ) {
        #[cfg(test)]
        if let Some(hook) = &self.checkpoint {
            hook(checkpoint, cancellation);
        }
        #[cfg(not(test))]
        let _ = cancellation;
    }

    fn record_scan_limit(&self, limit: Option<usize>) {
        #[cfg(test)]
        {
            if let Some(observed) = &self.scan_limits {
                observed.lock().unwrap().push(limit);
            }
            if let Some(observer) = &self.observer {
                observer.observe_scan(limit);
            }
        }
        #[cfg(not(test))]
        let _ = limit;
    }
}

type EvaluatedValue = Arc<DataFrame>;

struct Evaluator<'a> {
    backend: &'a ProductionRelationalBackend,
    context: &'a RelationalContext<'a>,
    plan: &'a CompiledRelationalPlan,
    operation_inputs: &'a [RuntimeValue],
    input_positions: BTreeMap<RelationalOperatorIndex, usize>,
    source_limits: BTreeMap<RelationalOperatorIndex, usize>,
    values: Vec<Option<EvaluatedValue>>,
}

impl<'a> Evaluator<'a> {
    fn new(
        backend: &'a ProductionRelationalBackend,
        context: &'a RelationalContext<'a>,
        plan: &'a CompiledRelationalPlan,
        operation_inputs: &'a [RuntimeValue],
    ) -> Self {
        let input_positions = plan
            .operators
            .iter()
            .enumerate()
            .filter_map(|(index, operator)| {
                matches!(operator, RelationalOperator::Input { .. })
                    .then_some(RelationalOperatorIndex::new(index as u32))
            })
            .enumerate()
            .map(|(position, index)| (index, position))
            .collect();
        Self {
            backend,
            context,
            plan,
            operation_inputs,
            input_positions,
            source_limits: source_limits(plan),
            values: vec![None; plan.operators.len()],
        }
    }

    fn evaluate(
        &mut self,
        index: RelationalOperatorIndex,
    ) -> Result<EvaluatedValue, RelationalError> {
        self.backend.checkpoint(
            #[cfg(test)]
            ProductionRelationalCheckpoint::OperatorEvaluation,
            self.context.cancellation,
        );
        self.context.check_terminal()?;
        if let Some(value) = self.values.get(index.index()).and_then(Clone::clone) {
            return Ok(value);
        }
        let operator = self
            .plan
            .operators
            .get(index.index())
            .ok_or_else(|| {
                RelationalError::new(
                    RelationalErrorCode::OperatorInvalid,
                    "relational operator index is invalid",
                )
            })?
            .clone();
        let value = match operator {
            RelationalOperator::Input { .. } => {
                let value = self
                    .input_positions
                    .get(&index)
                    .and_then(|position| self.operation_inputs.get(*position))
                    .cloned()
                    .ok_or_else(|| {
                        RelationalError::new(
                            RelationalErrorCode::InputShapeInvalid,
                            "relational input is missing",
                        )
                    })?;
                Arc::new(tabular_runtime_to_dataframe(value)?)
            }
            RelationalOperator::Source { resource, .. } => {
                self.backend.checkpoint(
                    #[cfg(test)]
                    ProductionRelationalCheckpoint::SourceScan,
                    self.context.cancellation,
                );
                self.context.check_terminal()?;
                let limit = self.source_limits.get(&index).copied();
                self.backend.record_scan_limit(limit);
                let lease = self
                    .context
                    .resources
                    .get(&resource)
                    .and_then(|lease| lease.as_any().downcast_ref::<ProjectResourceLease>())
                    .ok_or_else(|| {
                        RelationalError::new(
                            RelationalErrorCode::OperatorInvalid,
                            format!("relational source '{}' is unavailable", resource.as_str()),
                        )
                    })?;
                let scan = lease
                    .scan_dataframe(limit)
                    .map_err(|_| {
                        RelationalError::new(
                            RelationalErrorCode::OperatorInvalid,
                            "relational source scan failed",
                        )
                    })?
                    .ok_or_else(|| {
                        RelationalError::new(
                            RelationalErrorCode::OperatorInvalid,
                            format!("relational source '{}' is unavailable", resource.as_str()),
                        )
                    })?;
                scan.dataframe
            }
            RelationalOperator::Limit { input, rows } => {
                let dataframe = self.evaluate(input)?;
                Arc::new(dataframe.head(Some(rows.min(usize::MAX as u64) as usize)))
            }
            RelationalOperator::Project { input, columns } => {
                let source = self.evaluate(input)?;
                Arc::new(project_dataframe(source.as_ref().clone(), &columns)?)
            }
            RelationalOperator::Filter { input, predicate } => {
                let source = self.evaluate(input)?;
                self.backend.checkpoint(
                    #[cfg(test)]
                    ProductionRelationalCheckpoint::PredicateEvaluation,
                    self.context.cancellation,
                );
                self.context.check_terminal()?;
                let filtered = filter_dataframe(source.as_ref().clone(), &predicate)?;
                self.context.check_terminal()?;
                Arc::new(filtered)
            }
            RelationalOperator::Rename { input, columns } => {
                let source = self.evaluate(input)?;
                Arc::new(rename_dataframe(source.as_ref().clone(), &columns)?)
            }
            RelationalOperator::Union { inputs, all: _ } => {
                let mut inputs = inputs.iter();
                let first = inputs.next().ok_or_else(|| {
                    RelationalError::new(
                        RelationalErrorCode::OperatorInvalid,
                        "union requires at least one input",
                    )
                })?;
                let mut combined = self.evaluate(*first)?.as_ref().clone();
                for input in inputs {
                    let dataframe = self.evaluate(*input)?;
                    combined.vstack_mut(dataframe.as_ref()).map_err(|_| {
                        RelationalError::new(
                            RelationalErrorCode::TypeMismatch,
                            "union inputs have incompatible schemas",
                        )
                    })?;
                }
                Arc::new(combined)
            }
        };
        self.values[index.index()] = Some(value.clone());
        Ok(value)
    }

    fn materialize_index(
        &mut self,
        index: RelationalOperatorIndex,
    ) -> Result<RuntimeValue, RelationalError> {
        let value = self.evaluate(index)?;
        self.materialize(value)
    }

    fn materialize(&self, value: EvaluatedValue) -> Result<RuntimeValue, RelationalError> {
        self.backend.checkpoint(
            #[cfg(test)]
            ProductionRelationalCheckpoint::ResultMaterialization,
            self.context.cancellation,
        );
        self.context.check_terminal()?;
        #[cfg(test)]
        if let Some(observer) = &self.backend.observer {
            observer.observe_materialization(value.as_ref());
        }
        let converted = dataframe_to_protocol_value_with_checkpoint(value.as_ref(), || {
            self.backend.checkpoint(
                #[cfg(test)]
                ProductionRelationalCheckpoint::ResultConversion,
                self.context.cancellation,
            );
            self.context
                .check_terminal()
                .map_err(|error| match error.code() {
                    RelationalErrorCode::Cancelled => {
                        KernelError::cancelled("relational result conversion was cancelled")
                    }
                    RelationalErrorCode::DeadlineExceeded => KernelError::deadline_exceeded(),
                    _ => KernelError::new("relational result conversion failed"),
                })
        });
        self.context.check_terminal()?;
        converted.map(RuntimeValue::Scalar).map_err(|_| {
            RelationalError::new(
                RelationalErrorCode::TypeMismatch,
                "relational result conversion failed",
            )
        })
    }
}

impl RelationalBackend for ProductionRelationalBackend {
    fn execute(
        &self,
        context: &RelationalContext<'_>,
        plan: &CompiledRelationalPlan,
        operation_inputs: &[RuntimeValue],
    ) -> Result<RelationalExecution, RelationalError> {
        context.check_terminal()?;
        let inferred_pushdown_hints = infer_relational_pushdown_hints(&plan.operators, &plan.roots);
        if plan.pushdown_hints.as_ref() != inferred_pushdown_hints.as_slice() {
            return Err(RelationalError::new(
                RelationalErrorCode::HintInvalid,
                "compiled relational pushdown hints do not match operator inference",
            ));
        }
        #[cfg(test)]
        if let Some(observer) = &self.observer {
            observer.observe_invocation();
        }
        let mut evaluator = Evaluator::new(self, context, plan, operation_inputs);
        let mut outputs = Vec::with_capacity(plan.roots.len());
        for root in &plan.roots {
            outputs.push(evaluator.materialize_index(*root)?);
        }
        context.check_terminal()?;
        Ok(RelationalExecution { outputs })
    }
}

fn source_limits(plan: &CompiledRelationalPlan) -> BTreeMap<RelationalOperatorIndex, usize> {
    let mut requirements = BTreeMap::new();
    for root in &plan.roots {
        collect_source_requirements(plan, *root, None, &mut requirements);
    }

    requirements
        .into_iter()
        .filter_map(|(source, rows)| rows.map(|rows| (source, rows)))
        .collect()
}

fn collect_source_requirements(
    plan: &CompiledRelationalPlan,
    index: RelationalOperatorIndex,
    bound: Option<usize>,
    requirements: &mut BTreeMap<RelationalOperatorIndex, Option<usize>>,
) {
    let Some(operator) = plan.operators.get(index.index()) else {
        return;
    };
    match operator {
        RelationalOperator::Input { .. } => {}
        RelationalOperator::Source { .. } => {
            requirements
                .entry(index)
                .and_modify(|current| {
                    *current = match (*current, bound) {
                        (Some(current), Some(bound)) => Some(current.max(bound)),
                        _ => None,
                    };
                })
                .or_insert(bound);
        }
        RelationalOperator::Limit { input, rows } => {
            let hinted = plan.pushdown_hints.iter().any(|hint| {
                matches!(
                    hint,
                    crate::node_system::plan::RelationalPushdownHint::Limit {
                        source,
                        rows: hinted_rows,
                    } if source == input && hinted_rows == rows
                )
            });
            let bound = if hinted {
                let rows = (*rows).min(usize::MAX as u64) as usize;
                Some(bound.map_or(rows, |current| current.min(rows)))
            } else {
                bound
            };
            collect_source_requirements(plan, *input, bound, requirements);
        }
        RelationalOperator::Project { input, .. }
        | RelationalOperator::Filter { input, .. }
        | RelationalOperator::Rename { input, .. } => {
            collect_source_requirements(plan, *input, bound, requirements);
        }
        RelationalOperator::Union { inputs, .. } => {
            for input in inputs {
                collect_source_requirements(plan, *input, bound, requirements);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Evaluator, ProductionRelationalBackend, ProductionRelationalCheckpoint};
    use crate::node_system::analysis::{ProjectSessionId, RunId};
    use crate::node_system::plan::{
        CompiledRelationalPlan, CompiledResourceRequirement, RelationalExpression,
        RelationalFragmentId, RelationalFragmentRoot, RelationalLiteral, RelationalOperator,
        RelationalOperatorIndex, RelationalProjection, RelationalPushdownHint, RelationalRename,
        ResourceAccess, ResourceId, ResourceKind, infer_relational_pushdown_hints,
    };
    use crate::node_system::protocol::Value;
    use crate::node_system::runtime::{
        CancellationToken, ProjectResourceProvider, ProjectResourceSnapshot, RelationalBackend,
        RelationalContext, RelationalError, RelationalErrorCode, RunDeadline, RunResourceBudgets,
        RunResourceOwner, RunResourceSet, RuntimeValue,
    };
    use polars::prelude::{Column, DataFrame};
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn relational_test_owner(cancellation: &CancellationToken) -> RunResourceOwner {
        RunResourceOwner::new(
            RunId::new(1),
            RunResourceBudgets::default(),
            cancellation.clone(),
        )
        .unwrap()
    }

    #[test]
    fn rejects_forged_limit_pushdown_before_scanning_source() {
        let resource = ResourceId::new("databases/main").unwrap();
        let dataframe =
            DataFrame::new(4, vec![Column::new("value".into(), &[1_i64, 2, 3, 4])]).unwrap();
        let provider = ProjectResourceProvider::new(
            ProjectResourceSnapshot::new(ProjectSessionId::new("test"), Default::default())
                .with_database(resource.clone(), Arc::new(dataframe)),
        );
        let requirement = CompiledResourceRequirement {
            resource: resource.clone(),
            kind: ResourceKind::DatabaseConnection,
            access: ResourceAccess::Shared,
            optional: false,
        };
        let resources = RunResourceSet::acquire(&[requirement], &provider).unwrap();
        let plan = CompiledRelationalPlan {
            fragment_order: Box::new([]),
            operators: Box::new([
                RelationalOperator::Source {
                    resource,
                    relation: "main".into(),
                },
                RelationalOperator::Filter {
                    input: RelationalOperatorIndex::new(0),
                    predicate: RelationalExpression::Literal(RelationalLiteral::Boolean(true)),
                },
                RelationalOperator::Limit {
                    input: RelationalOperatorIndex::new(1),
                    rows: 2,
                },
            ]),
            fragment_roots: Box::new([]),
            roots: Box::new([RelationalOperatorIndex::new(2)]),
            pushdown_hints: Box::new([RelationalPushdownHint::Limit {
                source: RelationalOperatorIndex::new(0),
                rows: 2,
            }]),
        };
        let cancellation = CancellationToken::new();
        let resource_owner = relational_test_owner(&cancellation);
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            resource_owner: &resource_owner,
            cancellation: &cancellation,
            deadline: None,
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let backend = ProductionRelationalBackend::recording_scan_limits(Arc::clone(&observed));

        let error = backend.execute(&context, &plan, &[]).unwrap_err();

        assert_eq!(
            error.code(),
            crate::node_system::runtime::RelationalErrorCode::HintInvalid
        );
        assert!(error.to_string().contains("pushdown hints"));
        assert!(observed.lock().unwrap().is_empty());
    }

    #[test]
    fn source_fragment_and_limit_root_disable_limit_pushdown() {
        let resource = ResourceId::new("databases/main").unwrap();
        let dataframe =
            DataFrame::new(4, vec![Column::new("value".into(), &[1_i64, 2, 3, 4])]).unwrap();
        let provider = ProjectResourceProvider::new(
            ProjectResourceSnapshot::new(ProjectSessionId::new("test"), Default::default())
                .with_database(resource.clone(), Arc::new(dataframe)),
        );
        let requirement = CompiledResourceRequirement {
            resource: resource.clone(),
            kind: ResourceKind::DatabaseConnection,
            access: ResourceAccess::Shared,
            optional: false,
        };
        let resources = RunResourceSet::acquire(&[requirement], &provider).unwrap();
        let source = RelationalFragmentId::new("source").unwrap();
        let mut plan = CompiledRelationalPlan {
            fragment_order: Box::new([source.clone()]),
            operators: Box::new([
                RelationalOperator::Source {
                    resource,
                    relation: "main".into(),
                },
                RelationalOperator::Limit {
                    input: RelationalOperatorIndex::new(0),
                    rows: 2,
                },
            ]),
            fragment_roots: Box::new([RelationalFragmentRoot {
                fragment: source,
                operator: RelationalOperatorIndex::new(0),
            }]),
            roots: Box::new([
                RelationalOperatorIndex::new(1),
                RelationalOperatorIndex::new(0),
            ]),
            pushdown_hints: Box::new([]),
        };
        plan.pushdown_hints =
            infer_relational_pushdown_hints(&plan.operators, &plan.roots).into_boxed_slice();
        let cancellation = CancellationToken::new();
        let resource_owner = relational_test_owner(&cancellation);
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            resource_owner: &resource_owner,
            cancellation: &cancellation,
            deadline: None,
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let backend = ProductionRelationalBackend::recording_scan_limits(Arc::clone(&observed));

        let execution = backend.execute(&context, &plan, &[]).unwrap();

        assert_eq!(observed.lock().unwrap().as_slice(), &[None]);
        assert_eq!(output_len(&execution.outputs[0]), 2);
        assert_eq!(output_len(&execution.outputs[1]), 4);
    }

    #[test]
    fn multiple_limit_roots_push_down_largest_requested_bound() {
        let resource = ResourceId::new("databases/main").unwrap();
        let dataframe = DataFrame::new(
            12,
            vec![Column::new(
                "value".into(),
                &[1_i64, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
            )],
        )
        .unwrap();
        let provider = ProjectResourceProvider::new(
            ProjectResourceSnapshot::new(ProjectSessionId::new("test"), Default::default())
                .with_database(resource.clone(), Arc::new(dataframe)),
        );
        let requirement = CompiledResourceRequirement {
            resource: resource.clone(),
            kind: ResourceKind::DatabaseConnection,
            access: ResourceAccess::Shared,
            optional: false,
        };
        let resources = RunResourceSet::acquire(&[requirement], &provider).unwrap();
        let plan = CompiledRelationalPlan {
            fragment_order: Box::new([]),
            operators: Box::new([
                RelationalOperator::Source {
                    resource,
                    relation: "main".into(),
                },
                RelationalOperator::Limit {
                    input: RelationalOperatorIndex::new(0),
                    rows: 2,
                },
                RelationalOperator::Limit {
                    input: RelationalOperatorIndex::new(0),
                    rows: 10,
                },
            ]),
            fragment_roots: Box::new([]),
            roots: Box::new([
                RelationalOperatorIndex::new(1),
                RelationalOperatorIndex::new(2),
            ]),
            pushdown_hints: Box::new([
                RelationalPushdownHint::Limit {
                    source: RelationalOperatorIndex::new(0),
                    rows: 2,
                },
                RelationalPushdownHint::Limit {
                    source: RelationalOperatorIndex::new(0),
                    rows: 10,
                },
            ]),
        };
        let cancellation = CancellationToken::new();
        let resource_owner = relational_test_owner(&cancellation);
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            resource_owner: &resource_owner,
            cancellation: &cancellation,
            deadline: None,
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let backend = ProductionRelationalBackend::recording_scan_limits(Arc::clone(&observed));

        let execution = backend.execute(&context, &plan, &[]).unwrap();

        assert_eq!(observed.lock().unwrap().as_slice(), &[Some(10)]);
        assert_eq!(output_len(&execution.outputs[0]), 2);
        assert_eq!(output_len(&execution.outputs[1]), 10);
    }

    #[test]
    fn projection_and_predicate_hints_do_not_change_evaluated_values() {
        let predicate = RelationalExpression::Equal(
            Box::new(RelationalExpression::Column("status".into())),
            Box::new(RelationalExpression::Literal(RelationalLiteral::String(
                "paid".into(),
            ))),
        );
        let plan = CompiledRelationalPlan {
            fragment_order: Box::new([]),
            operators: Box::new([
                RelationalOperator::Input {
                    name: "source".into(),
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
            ]),
            fragment_roots: Box::new([]),
            roots: Box::new([RelationalOperatorIndex::new(2)]),
            pushdown_hints: Box::new([
                RelationalPushdownHint::Projection {
                    source: RelationalOperatorIndex::new(0),
                    columns: Box::new(["amount".into(), "status".into()]),
                },
                RelationalPushdownHint::Predicate {
                    source: RelationalOperatorIndex::new(0),
                    predicate,
                },
            ]),
        };
        let input = dataframe_value(&[
            ("amount", vec![Value::Integer(10), Value::Integer(20)]),
            ("status", vec![string("paid"), string("open")]),
        ]);

        let with_hints = evaluate_without_plan_validation(&plan, input.clone()).unwrap();
        let mut without_hints = plan;
        without_hints.pushdown_hints = Box::new([]);
        let without_hints = evaluate_without_plan_validation(&without_hints, input).unwrap();

        assert_eq!(with_hints, without_hints);
        assert_eq!(
            with_hints,
            dataframe_value(&[("amount", vec![Value::Integer(10)])])
        );
    }

    fn evaluate_without_plan_validation(
        plan: &CompiledRelationalPlan,
        input: RuntimeValue,
    ) -> Result<RuntimeValue, RelationalError> {
        let provider = ProjectResourceProvider::new(ProjectResourceSnapshot::new(
            ProjectSessionId::new("metadata-hint-test"),
            Default::default(),
        ));
        let resources = RunResourceSet::acquire(&[], &provider).unwrap();
        let cancellation = CancellationToken::new();
        let resource_owner = relational_test_owner(&cancellation);
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            resource_owner: &resource_owner,
            cancellation: &cancellation,
            deadline: None,
        };
        let backend = ProductionRelationalBackend::default();
        let operation_inputs = [input];
        let mut evaluator = Evaluator::new(&backend, &context, plan, &operation_inputs);
        evaluator.materialize_index(plan.roots[0])
    }

    #[test]
    fn explicit_adapter_artifact_ingress_stays_dataframe_native_through_relational_operators() {
        let predicate = RelationalExpression::GreaterThan(
            Box::new(RelationalExpression::Column("amount".into())),
            Box::new(RelationalExpression::Literal(RelationalLiteral::Integer(1))),
        );
        let mut plan = CompiledRelationalPlan {
            fragment_order: Box::new([]),
            operators: Box::new([
                RelationalOperator::Input {
                    name: "source".into(),
                },
                RelationalOperator::Filter {
                    input: RelationalOperatorIndex::new(0),
                    predicate,
                },
                RelationalOperator::Project {
                    input: RelationalOperatorIndex::new(1),
                    columns: Box::new([
                        RelationalProjection {
                            name: "city".into(),
                            expression: RelationalExpression::Column("city".into()),
                        },
                        RelationalProjection {
                            name: "amount".into(),
                            expression: RelationalExpression::Column("amount".into()),
                        },
                    ]),
                },
                RelationalOperator::Rename {
                    input: RelationalOperatorIndex::new(2),
                    columns: Box::new([rename("city", "location")]),
                },
            ]),
            fragment_roots: Box::new([]),
            roots: Box::new([RelationalOperatorIndex::new(3)]),
            pushdown_hints: Box::new([]),
        };
        plan.pushdown_hints =
            crate::node_system::plan::infer_relational_pushdown_hints(&plan.operators, &plan.roots)
                .into_boxed_slice();
        let input = dataframe_value(&[
            ("amount", vec![Value::Integer(1), Value::Integer(2)]),
            ("city", vec![string("Paris"), string("Tokyo")]),
        ]);
        let RuntimeValue::Scalar(value) = input else {
            unreachable!()
        };
        let artifact = RuntimeValue::Artifact(crate::node_system::runtime::Artifact::new(
            crate::node_system::runtime::ArtifactKind::Collected,
            [value],
        ));

        let output =
            execute_input_plan(&plan, artifact, ProductionRelationalBackend::default()).unwrap();

        assert_eq!(
            output,
            dataframe_value(&[
                ("amount", vec![Value::Integer(2)]),
                ("location", vec![string("Tokyo")]),
            ])
        );
    }

    #[test]
    fn observer_sees_dataframe_schema_order_and_nulls_before_external_conversion() {
        let mut plan = CompiledRelationalPlan {
            fragment_order: Box::new([]),
            operators: Box::new([
                RelationalOperator::Input {
                    name: "source".into(),
                },
                RelationalOperator::Project {
                    input: RelationalOperatorIndex::new(0),
                    columns: Box::new([
                        RelationalProjection {
                            name: "z".into(),
                            expression: RelationalExpression::Column("z".into()),
                        },
                        RelationalProjection {
                            name: "a".into(),
                            expression: RelationalExpression::Column("a".into()),
                        },
                    ]),
                },
            ]),
            fragment_roots: Box::new([]),
            roots: Box::new([RelationalOperatorIndex::new(1)]),
            pushdown_hints: Box::new([]),
        };
        plan.pushdown_hints =
            crate::node_system::plan::infer_relational_pushdown_hints(&plan.operators, &plan.roots)
                .into_boxed_slice();
        let observer = Arc::new(super::ProductionRelationalObserver::default());
        let backend = ProductionRelationalBackend::with_observer(Arc::clone(&observer));

        let output = execute_input_plan(
            &plan,
            dataframe_value(&[
                ("a", vec![Value::Integer(1), Value::Null]),
                ("z", vec![string("first"), string("second")]),
            ]),
            backend,
        )
        .unwrap();

        let observed = observer.materialized_dataframes();
        assert_eq!(observed.len(), 1);
        assert_eq!(
            observed[0]
                .get_column_names()
                .into_iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["z", "a"]
        );
        assert_eq!(observed[0]["z"].dtype(), &polars::prelude::DataType::String);
        assert_eq!(observed[0]["a"].dtype(), &polars::prelude::DataType::Int64);
        assert_eq!(observed[0]["a"].null_count(), 1);
        let RuntimeValue::Scalar(Value::Object(columns)) = output else {
            panic!("external result shape changed")
        };
        assert_eq!(
            columns.keys().map(AsRef::as_ref).collect::<Vec<_>>(),
            vec!["a", "z"]
        );
    }

    fn execute_input_plan(
        plan: &CompiledRelationalPlan,
        input: RuntimeValue,
        backend: ProductionRelationalBackend,
    ) -> Result<RuntimeValue, RelationalError> {
        let provider = ProjectResourceProvider::new(ProjectResourceSnapshot::new(
            ProjectSessionId::new("dataframe-input-test"),
            Default::default(),
        ));
        let resources = RunResourceSet::acquire(&[], &provider).unwrap();
        let cancellation = CancellationToken::new();
        let resource_owner = relational_test_owner(&cancellation);
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            resource_owner: &resource_owner,
            cancellation: &cancellation,
            deadline: None,
        };
        backend
            .execute(&context, plan, &[input])
            .map(|mut execution| execution.outputs.remove(0))
    }

    #[test]
    fn rename_preserves_values_names_untouched_columns_and_row_count() {
        let output = execute_rename(
            dataframe_value(&[
                ("city", vec![string("Paris"), string("Tokyo")]),
                ("sales", vec![Value::Integer(10), Value::Integer(20)]),
            ]),
            vec![rename("city", "location")],
        )
        .unwrap();

        assert_eq!(
            output,
            dataframe_value(&[
                ("location", vec![string("Paris"), string("Tokyo")]),
                ("sales", vec![Value::Integer(10), Value::Integer(20)]),
            ])
        );
    }

    #[test]
    fn rename_rejects_blank_source_name_before_exposing_any_output() {
        let error = execute_rename(
            dataframe_value(&[
                ("", vec![string("private")]),
                ("city", vec![string("Paris")]),
            ]),
            vec![rename("city", "location"), rename("", "redacted")],
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "rename source column name must not be empty"
        );
    }

    #[test]
    fn rename_rejects_padded_source_name_before_exposing_any_output() {
        let error = execute_rename(
            dataframe_value(&[
                (" city", vec![string("private")]),
                ("city", vec![string("Paris")]),
            ]),
            vec![rename("city", "location"), rename(" city", "redacted")],
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "rename source column name must not have leading or trailing whitespace"
        );
    }

    #[test]
    fn rename_rejects_missing_source_before_applying_any_mapping() {
        let error = execute_rename(
            dataframe_value(&[
                ("city", vec![string("Paris"), string("Tokyo")]),
                ("sales", vec![Value::Integer(10), Value::Integer(20)]),
            ]),
            vec![rename("city", "location"), rename("missing", "other")],
        )
        .unwrap_err();

        assert!(error.to_string().contains("missing"));
    }

    #[test]
    fn rename_rejects_destination_collision_before_applying_any_mapping() {
        let error = execute_rename(
            dataframe_value(&[
                ("city", vec![string("Paris"), string("Tokyo")]),
                ("sales", vec![Value::Integer(10), Value::Integer(20)]),
                ("region", vec![string("EU"), string("APAC")]),
            ]),
            vec![rename("city", "location"), rename("sales", "region")],
        )
        .unwrap_err();

        assert!(error.to_string().contains("region"));
    }

    #[test]
    fn rename_rejects_duplicate_source_mappings() {
        let error = execute_rename(
            dataframe_value(&[("city", vec![string("Paris")])]),
            vec![rename("city", "location"), rename("city", "place")],
        )
        .unwrap_err();

        assert!(error.to_string().contains("city"));
    }

    #[test]
    fn rename_rejects_duplicate_destination_mappings() {
        let error = execute_rename(
            dataframe_value(&[
                ("city", vec![string("Paris")]),
                ("region", vec![string("EU")]),
            ]),
            vec![rename("city", "place"), rename("region", "place")],
        )
        .unwrap_err();

        assert!(error.to_string().contains("place"));
    }

    #[test]
    fn rename_validates_against_original_fields_and_applies_swaps_atomically() {
        let output = execute_rename(
            dataframe_value(&[
                ("city", vec![string("Paris"), string("Tokyo")]),
                ("region", vec![string("EU"), string("APAC")]),
            ]),
            vec![rename("city", "region"), rename("region", "city")],
        )
        .unwrap();

        assert_eq!(
            output,
            dataframe_value(&[
                ("city", vec![string("EU"), string("APAC")]),
                ("region", vec![string("Paris"), string("Tokyo")]),
            ])
        );
    }

    #[test]
    fn rename_same_name_is_a_no_op() {
        let input = dataframe_value(&[
            ("city", vec![string("Paris"), string("Tokyo")]),
            ("sales", vec![Value::Integer(10), Value::Integer(20)]),
        ]);

        let output = execute_rename(input.clone(), vec![rename("city", "city")]).unwrap();

        assert_eq!(output, input);
    }

    fn execute_rename(
        input: RuntimeValue,
        columns: Vec<RelationalRename>,
    ) -> Result<RuntimeValue, RelationalError> {
        let provider = ProjectResourceProvider::new(ProjectResourceSnapshot::new(
            ProjectSessionId::new("rename-test"),
            Default::default(),
        ));
        let resources = RunResourceSet::acquire(&[], &provider).unwrap();
        let plan = CompiledRelationalPlan {
            fragment_order: Box::new([]),
            operators: vec![
                RelationalOperator::Input {
                    name: "source".into(),
                },
                RelationalOperator::Rename {
                    input: RelationalOperatorIndex::new(0),
                    columns: columns.into_boxed_slice(),
                },
            ]
            .into_boxed_slice(),
            fragment_roots: Box::new([]),
            roots: Box::new([RelationalOperatorIndex::new(1)]),
            pushdown_hints: Box::new([]),
        };
        let cancellation = CancellationToken::new();
        let resource_owner = relational_test_owner(&cancellation);
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            resource_owner: &resource_owner,
            cancellation: &cancellation,
            deadline: None,
        };

        ProductionRelationalBackend::default()
            .execute(&context, &plan, &[input])
            .map(|mut execution| execution.outputs.remove(0))
    }

    fn dataframe_value(columns: &[(&str, Vec<Value>)]) -> RuntimeValue {
        RuntimeValue::Scalar(Value::Object(
            columns
                .iter()
                .map(|(name, values)| ((*name).into(), Value::List(values.clone())))
                .collect::<BTreeMap<_, _>>(),
        ))
    }

    fn string(value: &str) -> Value {
        Value::String(value.into())
    }

    fn rename(from: &str, to: &str) -> RelationalRename {
        RelationalRename {
            from: from.into(),
            to: to.into(),
        }
    }

    fn output_len(value: &RuntimeValue) -> usize {
        let RuntimeValue::Scalar(Value::Object(columns)) = value else {
            panic!("expected a materialized dataframe output");
        };
        let Value::List(values) = &columns["value"] else {
            panic!("expected a dataframe column");
        };
        values.len()
    }

    #[test]
    fn root_only_execution_does_not_scan_unreachable_source() {
        let resource = ResourceId::new("databases/main").unwrap();
        let unavailable = ResourceId::new("databases/unreachable").unwrap();
        let dataframe =
            DataFrame::new(4, vec![Column::new("value".into(), &[1_i64, 2, 3, 4])]).unwrap();
        let provider = ProjectResourceProvider::new(
            ProjectResourceSnapshot::new(ProjectSessionId::new("test"), Default::default())
                .with_database(resource.clone(), Arc::new(dataframe)),
        );
        let requirement = CompiledResourceRequirement {
            resource: resource.clone(),
            kind: ResourceKind::DatabaseConnection,
            access: ResourceAccess::Shared,
            optional: false,
        };
        let resources = RunResourceSet::acquire(&[requirement], &provider).unwrap();
        let plan = CompiledRelationalPlan {
            fragment_order: Box::new([]),
            operators: Box::new([
                RelationalOperator::Source {
                    resource,
                    relation: "main".into(),
                },
                RelationalOperator::Source {
                    resource: unavailable,
                    relation: "unreachable".into(),
                },
            ]),
            fragment_roots: Box::new([]),
            roots: Box::new([RelationalOperatorIndex::new(0)]),
            pushdown_hints: Box::new([]),
        };
        let cancellation = CancellationToken::new();
        let resource_owner = relational_test_owner(&cancellation);
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            resource_owner: &resource_owner,
            cancellation: &cancellation,
            deadline: None,
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let backend = ProductionRelationalBackend::recording_scan_limits(Arc::clone(&observed));

        let execution = backend.execute(&context, &plan, &[]).unwrap();

        assert_eq!(observed.lock().unwrap().as_slice(), &[None]);
        assert_eq!(output_len(&execution.outputs[0]), 4);
    }

    #[test]
    fn cancellation_at_result_materialization_stops_inside_backend() {
        let provider = ProjectResourceProvider::new(ProjectResourceSnapshot::new(
            ProjectSessionId::new("test"),
            Default::default(),
        ));
        let resources = RunResourceSet::acquire(&[], &provider).unwrap();
        let fragment = RelationalFragmentId::new("input").unwrap();
        let plan = CompiledRelationalPlan {
            fragment_order: Box::new([fragment.clone()]),
            operators: Box::new([RelationalOperator::Input {
                name: "input".into(),
            }]),
            fragment_roots: Box::new([RelationalFragmentRoot {
                fragment,
                operator: RelationalOperatorIndex::new(0),
            }]),
            roots: Box::new([RelationalOperatorIndex::new(0)]),
            pushdown_hints: Box::new([]),
        };
        let cancellation = CancellationToken::new();
        let resource_owner = relational_test_owner(&cancellation);
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            resource_owner: &resource_owner,
            cancellation: &cancellation,
            deadline: None,
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let observed_for_hook = Arc::clone(&observed);
        let backend = ProductionRelationalBackend::default().with_test_checkpoint(Arc::new(
            move |checkpoint, cancellation| {
                observed_for_hook.lock().unwrap().push(checkpoint);
                if checkpoint == ProductionRelationalCheckpoint::ResultMaterialization {
                    cancellation.cancel();
                }
            },
        ));

        let result = backend.execute(
            &context,
            &plan,
            &[RuntimeValue::Scalar(Value::Object(BTreeMap::new()))],
        );

        assert_eq!(
            result.unwrap_err().code(),
            crate::node_system::runtime::RelationalErrorCode::Cancelled
        );
        assert_eq!(
            observed.lock().unwrap().as_slice(),
            &[
                ProductionRelationalCheckpoint::OperatorEvaluation,
                ProductionRelationalCheckpoint::ResultMaterialization,
            ]
        );
    }

    #[test]
    fn production_backend_observes_deadline_after_blocking_checkpoint() {
        let provider = ProjectResourceProvider::new(ProjectResourceSnapshot::new(
            ProjectSessionId::new("deadline-test"),
            Default::default(),
        ));
        let resources = RunResourceSet::acquire(&[], &provider).unwrap();
        let fragment = RelationalFragmentId::new("input").unwrap();
        let plan = CompiledRelationalPlan {
            fragment_order: Box::new([fragment.clone()]),
            operators: Box::new([RelationalOperator::Input {
                name: "input".into(),
            }]),
            fragment_roots: Box::new([RelationalFragmentRoot {
                fragment,
                operator: RelationalOperatorIndex::new(0),
            }]),
            roots: Box::new([RelationalOperatorIndex::new(0)]),
            pushdown_hints: Box::new([]),
        };
        let cancellation = CancellationToken::new();
        let resource_owner = relational_test_owner(&cancellation);
        let deadline = RunDeadline::after(Duration::from_millis(5));
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            resource_owner: &resource_owner,
            cancellation: &cancellation,
            deadline: Some(deadline),
        };
        let backend = ProductionRelationalBackend::default().with_test_checkpoint(Arc::new(
            |checkpoint, _| {
                if checkpoint == ProductionRelationalCheckpoint::ResultMaterialization {
                    std::thread::sleep(Duration::from_millis(20));
                }
            },
        ));

        let error = backend
            .execute(
                &context,
                &plan,
                &[RuntimeValue::Scalar(Value::Object(BTreeMap::new()))],
            )
            .unwrap_err();

        assert_eq!(error.code(), RelationalErrorCode::DeadlineExceeded);
    }

    #[test]
    fn source_limit_pushdown_bounds_scan_before_materialization() {
        let resource = ResourceId::new("databases/main").unwrap();
        let dataframe =
            DataFrame::new(4, vec![Column::new("value".into(), &[1_i64, 2, 3, 4])]).unwrap();
        let provider = ProjectResourceProvider::new(
            ProjectResourceSnapshot::new(ProjectSessionId::new("test"), Default::default())
                .with_database(resource.clone(), Arc::new(dataframe)),
        );
        let requirement = CompiledResourceRequirement {
            resource: resource.clone(),
            kind: ResourceKind::DatabaseConnection,
            access: ResourceAccess::Shared,
            optional: false,
        };
        let resources = RunResourceSet::acquire(&[requirement], &provider).unwrap();
        let source = RelationalFragmentId::new("source").unwrap();
        let limit = RelationalFragmentId::new("limit").unwrap();
        let plan = CompiledRelationalPlan {
            fragment_order: Box::new([source.clone(), limit.clone()]),
            operators: Box::new([
                RelationalOperator::Source {
                    resource,
                    relation: "main".into(),
                },
                RelationalOperator::Limit {
                    input: RelationalOperatorIndex::new(0),
                    rows: 2,
                },
            ]),
            fragment_roots: Box::new([
                RelationalFragmentRoot {
                    fragment: source,
                    operator: RelationalOperatorIndex::new(0),
                },
                RelationalFragmentRoot {
                    fragment: limit,
                    operator: RelationalOperatorIndex::new(1),
                },
            ]),
            roots: Box::new([RelationalOperatorIndex::new(1)]),
            pushdown_hints: Box::new([RelationalPushdownHint::Limit {
                source: RelationalOperatorIndex::new(0),
                rows: 2,
            }]),
        };
        let cancellation = CancellationToken::new();
        let resource_owner = relational_test_owner(&cancellation);
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            resource_owner: &resource_owner,
            cancellation: &cancellation,
            deadline: None,
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let backend = ProductionRelationalBackend::recording_scan_limits(Arc::clone(&observed));

        let execution = backend.execute(&context, &plan, &[]).unwrap();

        assert_eq!(observed.lock().unwrap().as_slice(), &[Some(2)]);
        let RuntimeValue::Scalar(Value::Object(columns)) = &execution.outputs[0] else {
            panic!("expected a materialized dataframe output");
        };
        let Value::List(values) = &columns["value"] else {
            panic!("expected a dataframe column");
        };
        assert_eq!(values.len(), 2);
    }
}
