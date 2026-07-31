use super::production_relational_value::{
    limit_protocol_value, relational_expression, relational_filter, relational_object,
    runtime_scalar,
};
use super::{
    CancellationToken, ProjectResourceLease, RelationalBackend, RelationalContext, RelationalError,
    RelationalExecution, RelationalInput, RuntimeValue, dataframe_to_protocol_value,
};
use crate::node_system::plan::{
    CompiledRelationalPlan, PlannedMaterializationBridge, RelationalOperator,
    RelationalOperatorIndex, infer_relational_pushdown_hints,
};
use crate::node_system::protocol::Value;
use polars::prelude::DataFrame;
use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductionRelationalCheckpoint {
    OperatorEvaluation,
    SourceScan,
    ResultMaterialization,
}

#[cfg(test)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProductionRelationalObservation {
    pub relational_islands: Option<usize>,
    pub materialization_bridges: Option<usize>,
    pub backend_invocations: usize,
    pub bridge_inputs: Vec<usize>,
    pub scan_limits: Vec<Option<usize>>,
}

#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct ProductionRelationalObserver {
    observation: std::sync::Mutex<ProductionRelationalObservation>,
}

#[cfg(test)]
impl ProductionRelationalObserver {
    pub(crate) fn observe_plan(&self, plan: &crate::node_system::plan::ExecutionPlan) {
        let mut observation = self.observation.lock().unwrap();
        observation.relational_islands = Some(plan.relational_subplans.len());
        observation.materialization_bridges = Some(
            plan.relational_subplans
                .iter()
                .map(|subplan| subplan.materialization_bridges.len())
                .sum(),
        );
    }

    fn observe_invocation(&self, bridge_inputs: usize) {
        let mut observation = self.observation.lock().unwrap();
        observation.backend_invocations += 1;
        observation.bridge_inputs.push(bridge_inputs);
    }

    fn observe_scan(&self, limit: Option<usize>) {
        self.observation.lock().unwrap().scan_limits.push(limit);
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

#[derive(Clone)]
enum EvaluatedValue {
    DataFrame(Arc<DataFrame>),
    Runtime(RuntimeValue),
}

struct Evaluator<'a> {
    backend: &'a ProductionRelationalBackend,
    context: &'a RelationalContext<'a>,
    plan: &'a CompiledRelationalPlan,
    operation_inputs: &'a [RuntimeValue],
    bridge_inputs: &'a [RelationalInput],
    bridges_by_operator: BTreeMap<RelationalOperatorIndex, &'a PlannedMaterializationBridge>,
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
        bridge_inputs: &'a [RelationalInput],
    ) -> Self {
        let bridges_by_operator = plan
            .bridge_inputs
            .iter()
            .map(|binding| (binding.operator, &binding.bridge))
            .collect::<BTreeMap<_, _>>();
        let input_positions = plan
            .operators
            .iter()
            .enumerate()
            .filter_map(|(index, operator)| {
                let index = RelationalOperatorIndex::new(index as u32);
                (matches!(operator, RelationalOperator::Input { .. })
                    && !bridges_by_operator.contains_key(&index))
                .then_some(index)
            })
            .enumerate()
            .map(|(position, index)| (index, position))
            .collect();
        Self {
            backend,
            context,
            plan,
            operation_inputs,
            bridge_inputs,
            bridges_by_operator,
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
        self.context
            .cancellation
            .check()
            .map_err(RelationalError::from)?;
        if let Some(value) = self.values.get(index.index()).and_then(Clone::clone) {
            return Ok(value);
        }
        let operator = self
            .plan
            .operators
            .get(index.index())
            .ok_or_else(|| RelationalError::new("relational operator index is invalid"))?
            .clone();
        let value = match operator {
            RelationalOperator::Input { .. } => {
                let value = if let Some(bridge) = self.bridges_by_operator.get(&index) {
                    self.bridge_inputs
                        .iter()
                        .find(|input| &input.bridge == *bridge)
                        .map(|input| input.value.clone())
                } else {
                    self.input_positions
                        .get(&index)
                        .and_then(|position| self.operation_inputs.get(*position))
                        .cloned()
                }
                .ok_or_else(|| RelationalError::new("relational input is missing"))?;
                EvaluatedValue::Runtime(value)
            }
            RelationalOperator::Source { resource, .. } => {
                self.backend.checkpoint(
                    #[cfg(test)]
                    ProductionRelationalCheckpoint::SourceScan,
                    self.context.cancellation,
                );
                self.context
                    .cancellation
                    .check()
                    .map_err(RelationalError::from)?;
                let limit = self.source_limits.get(&index).copied();
                self.backend.record_scan_limit(limit);
                let lease = self
                    .context
                    .resources
                    .get(&resource)
                    .and_then(|lease| lease.as_any().downcast_ref::<ProjectResourceLease>())
                    .ok_or_else(|| {
                        RelationalError::new(format!(
                            "relational source '{}' is unavailable",
                            resource.as_str()
                        ))
                    })?;
                let scan = lease
                    .scan_dataframe(limit)
                    .map_err(RelationalError::new)?
                    .ok_or_else(|| {
                        RelationalError::new(format!(
                            "relational source '{}' is unavailable",
                            resource.as_str()
                        ))
                    })?;
                EvaluatedValue::DataFrame(scan.dataframe)
            }
            RelationalOperator::Limit { input, rows } => match self.evaluate(input)? {
                EvaluatedValue::DataFrame(dataframe) => EvaluatedValue::DataFrame(Arc::new(
                    dataframe.head(Some(rows.min(usize::MAX as u64) as usize)),
                )),
                EvaluatedValue::Runtime(value) => EvaluatedValue::Runtime(RuntimeValue::Scalar(
                    limit_protocol_value(runtime_scalar(&value)?, rows)?,
                )),
            },
            RelationalOperator::Project { input, columns } => {
                let source = self.materialize_index(input)?;
                let mut projected = BTreeMap::new();
                for column in columns {
                    projected.insert(
                        column.name,
                        relational_expression(&column.expression, runtime_scalar(&source)?)?,
                    );
                }
                EvaluatedValue::Runtime(RuntimeValue::Scalar(Value::Object(projected)))
            }
            RelationalOperator::Filter { input, predicate } => {
                let source = self.materialize_index(input)?;
                let source = runtime_scalar(&source)?;
                let mask = relational_expression(&predicate, source)?;
                EvaluatedValue::Runtime(RuntimeValue::Scalar(relational_filter(source, &mask)?))
            }
            RelationalOperator::Rename { input, columns } => {
                let source = self.materialize_index(input)?;
                let mut source = relational_object(runtime_scalar(&source)?)?;
                for rename in columns {
                    if let Some(value) = source.remove(rename.from.as_ref()) {
                        source.insert(rename.to, value);
                    }
                }
                EvaluatedValue::Runtime(RuntimeValue::Scalar(Value::Object(source)))
            }
            RelationalOperator::Union { inputs, all: _ } => {
                let mut combined = BTreeMap::<Box<str>, Vec<Value>>::new();
                for input in inputs {
                    let value = self.materialize_index(input)?;
                    for (name, value) in relational_object(runtime_scalar(&value)?)? {
                        let Value::List(column) = value else {
                            return Err(RelationalError::new("union expects dataframe columns"));
                        };
                        combined.entry(name).or_default().extend(column);
                    }
                }
                EvaluatedValue::Runtime(RuntimeValue::Scalar(Value::Object(
                    combined
                        .into_iter()
                        .map(|(name, values)| (name, Value::List(values)))
                        .collect(),
                )))
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
        self.context
            .cancellation
            .check()
            .map_err(RelationalError::from)?;
        match value {
            EvaluatedValue::Runtime(value) => Ok(value),
            EvaluatedValue::DataFrame(dataframe) => dataframe_to_protocol_value(dataframe.as_ref())
                .map(RuntimeValue::Scalar)
                .map_err(|error| RelationalError::new(error.to_string())),
        }
    }
}

impl RelationalBackend for ProductionRelationalBackend {
    fn execute(
        &self,
        context: &RelationalContext<'_>,
        plan: &CompiledRelationalPlan,
        operation_inputs: &[RuntimeValue],
        bridge_inputs: &[RelationalInput],
    ) -> Result<RelationalExecution, RelationalError> {
        let inferred_pushdown_hints = infer_relational_pushdown_hints(&plan.operators);
        if plan.pushdown_hints.as_ref() != inferred_pushdown_hints.as_slice() {
            return Err(RelationalError::new(
                "compiled relational pushdown hints do not match operator inference",
            ));
        }
        #[cfg(test)]
        if let Some(observer) = &self.observer {
            observer.observe_invocation(bridge_inputs.len());
        }
        let requested_fragment_roots = plan
            .requested_fragment_outputs
            .iter()
            .map(|fragment| {
                let root = plan
                    .fragment_roots
                    .iter()
                    .find(|root| &root.fragment == fragment)
                    .map(|root| root.operator)
                    .ok_or_else(|| {
                        RelationalError::new(format!(
                            "requested relational fragment '{}' has no compiled root",
                            fragment.as_str()
                        ))
                    })?;
                Ok((fragment.clone(), root))
            })
            .collect::<Result<Vec<_>, RelationalError>>()?;
        let mut evaluator = Evaluator::new(self, context, plan, operation_inputs, bridge_inputs);
        let mut outputs = Vec::with_capacity(plan.roots.len());
        for root in &plan.roots {
            outputs.push(evaluator.materialize_index(*root)?);
        }
        let mut fragment_outputs = BTreeMap::new();
        for (fragment, root) in requested_fragment_roots {
            fragment_outputs.insert(fragment, evaluator.materialize_index(root)?);
        }
        Ok(RelationalExecution {
            outputs,
            fragment_outputs,
        })
    }
}

fn source_limits(plan: &CompiledRelationalPlan) -> BTreeMap<RelationalOperatorIndex, usize> {
    let mut requirements = BTreeMap::new();
    for root in &plan.roots {
        collect_source_requirements(plan, *root, None, &mut requirements);
    }
    for fragment in &plan.requested_fragment_outputs {
        if let Some(root) = plan
            .fragment_roots
            .iter()
            .find(|root| &root.fragment == fragment)
        {
            collect_source_requirements(plan, root.operator, None, &mut requirements);
        }
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
    use super::{ProductionRelationalBackend, ProductionRelationalCheckpoint};
    use crate::node_system::analysis::{ProjectSessionId, RunId};
    use crate::node_system::plan::{
        CompiledRelationalPlan, CompiledResourceRequirement, RelationalExpression,
        RelationalFragmentId, RelationalFragmentRoot, RelationalLiteral, RelationalOperator,
        RelationalOperatorIndex, RelationalPushdownHint, ResourceAccess, ResourceId, ResourceKind,
    };
    use crate::node_system::protocol::Value;
    use crate::node_system::runtime::{
        CancellationToken, ProjectResourceProvider, ProjectResourceSnapshot, RelationalBackend,
        RelationalContext, RunResourceSet, RuntimeValue,
    };
    use polars::prelude::{Column, DataFrame};
    use std::sync::{Arc, Mutex};

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
            bridge_inputs: Box::new([]),
            requested_fragment_outputs: Box::new([]),
            roots: Box::new([RelationalOperatorIndex::new(2)]),
            pushdown_hints: Box::new([RelationalPushdownHint::Limit {
                source: RelationalOperatorIndex::new(0),
                rows: 2,
            }]),
        };
        let cancellation = CancellationToken::new();
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            cancellation: &cancellation,
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let backend = ProductionRelationalBackend::recording_scan_limits(Arc::clone(&observed));

        let error = backend.execute(&context, &plan, &[], &[]).unwrap_err();

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
        let plan = CompiledRelationalPlan {
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
                fragment: source.clone(),
                operator: RelationalOperatorIndex::new(0),
            }]),
            bridge_inputs: Box::new([]),
            requested_fragment_outputs: Box::new([source.clone()]),
            roots: Box::new([RelationalOperatorIndex::new(1)]),
            pushdown_hints: Box::new([RelationalPushdownHint::Limit {
                source: RelationalOperatorIndex::new(0),
                rows: 2,
            }]),
        };
        let cancellation = CancellationToken::new();
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            cancellation: &cancellation,
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let backend = ProductionRelationalBackend::recording_scan_limits(Arc::clone(&observed));

        let execution = backend.execute(&context, &plan, &[], &[]).unwrap();

        assert_eq!(observed.lock().unwrap().as_slice(), &[None]);
        assert_eq!(output_len(&execution.outputs[0]), 2);
        assert_eq!(output_len(&execution.fragment_outputs[&source]), 4);
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
            bridge_inputs: Box::new([]),
            requested_fragment_outputs: Box::new([]),
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
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            cancellation: &cancellation,
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let backend = ProductionRelationalBackend::recording_scan_limits(Arc::clone(&observed));

        let execution = backend.execute(&context, &plan, &[], &[]).unwrap();

        assert_eq!(observed.lock().unwrap().as_slice(), &[Some(10)]);
        assert_eq!(output_len(&execution.outputs[0]), 2);
        assert_eq!(output_len(&execution.outputs[1]), 10);
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
            bridge_inputs: Box::new([]),
            requested_fragment_outputs: Box::new([]),
            roots: Box::new([RelationalOperatorIndex::new(0)]),
            pushdown_hints: Box::new([]),
        };
        let cancellation = CancellationToken::new();
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            cancellation: &cancellation,
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let backend = ProductionRelationalBackend::recording_scan_limits(Arc::clone(&observed));

        let execution = backend.execute(&context, &plan, &[], &[]).unwrap();

        assert_eq!(observed.lock().unwrap().as_slice(), &[None]);
        assert_eq!(output_len(&execution.outputs[0]), 4);
    }

    #[test]
    fn evaluates_only_requested_fragment_outputs() {
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
                    fragment: limit.clone(),
                    operator: RelationalOperatorIndex::new(1),
                },
            ]),
            bridge_inputs: Box::new([]),
            requested_fragment_outputs: Box::new([limit.clone()]),
            roots: Box::new([]),
            pushdown_hints: Box::new([RelationalPushdownHint::Limit {
                source: RelationalOperatorIndex::new(0),
                rows: 2,
            }]),
        };
        let cancellation = CancellationToken::new();
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            cancellation: &cancellation,
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let backend = ProductionRelationalBackend::recording_scan_limits(Arc::clone(&observed));

        let execution = backend.execute(&context, &plan, &[], &[]).unwrap();

        assert!(execution.outputs.is_empty());
        assert_eq!(
            execution.fragment_outputs.keys().collect::<Vec<_>>(),
            vec![&limit]
        );
        assert_eq!(output_len(&execution.fragment_outputs[&limit]), 2);
        assert_eq!(observed.lock().unwrap().as_slice(), &[Some(2)]);
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
            bridge_inputs: Box::new([]),
            requested_fragment_outputs: Box::new([]),
            roots: Box::new([RelationalOperatorIndex::new(0)]),
            pushdown_hints: Box::new([]),
        };
        let cancellation = CancellationToken::new();
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            cancellation: &cancellation,
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

        let result = backend.execute(&context, &plan, &[Value::Integer(1).into()], &[]);

        assert!(result.is_err());
        assert_eq!(
            observed.lock().unwrap().as_slice(),
            &[
                ProductionRelationalCheckpoint::OperatorEvaluation,
                ProductionRelationalCheckpoint::ResultMaterialization,
            ]
        );
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
            bridge_inputs: Box::new([]),
            requested_fragment_outputs: Box::new([]),
            roots: Box::new([RelationalOperatorIndex::new(1)]),
            pushdown_hints: Box::new([RelationalPushdownHint::Limit {
                source: RelationalOperatorIndex::new(0),
                rows: 2,
            }]),
        };
        let cancellation = CancellationToken::new();
        let context = RelationalContext {
            run_id: RunId::new(1),
            resources: &resources,
            cancellation: &cancellation,
        };
        let observed = Arc::new(Mutex::new(Vec::new()));
        let backend = ProductionRelationalBackend::recording_scan_limits(Arc::clone(&observed));

        let execution = backend.execute(&context, &plan, &[], &[]).unwrap();

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
