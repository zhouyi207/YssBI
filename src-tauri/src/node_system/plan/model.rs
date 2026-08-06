use crate::node_system::analysis::CompileProvenance;
use crate::node_system::document::{FunctionParameterId, GraphResourcePath, NodeId, PortAddress};
use crate::node_system::protocol::{
    CanonicalDecimal, InputConsumption, NodeTypeId, OutputProduction, Value,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

macro_rules! index_type {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(index: u32) -> Self {
                Self(index)
            }

            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }

        impl From<u32> for $name {
            fn from(index: u32) -> Self {
                Self::new(index)
            }
        }
    };
}

index_type!(OperationIndex);
index_type!(ValueRef);
index_type!(RelationalSubplanIndex);
index_type!(RelationalOperatorIndex);

macro_rules! opaque_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Box<str>);

        impl $name {
            pub fn new(value: impl Into<Box<str>>) -> Result<Self, InvalidPlanId> {
                let value = value.into();
                if value.is_empty() || value.trim() != value.as_ref() {
                    return Err(InvalidPlanId);
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

opaque_id!(KernelHandle);
opaque_id!(CompiledParameterHandle);
opaque_id!(FunctionPlanHandle);
opaque_id!(RelationalBackendId);
opaque_id!(RelationalFragmentId);
opaque_id!(ResourceId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidPlanId;

impl fmt::Display for InvalidPlanId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("plan identifiers must be non-empty and have no surrounding whitespace")
    }
}

impl std::error::Error for InvalidPlanId {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GraphOutputRef {
    pub graph_path: GraphResourcePath,
    pub port: PortAddress,
}

macro_rules! define_execution_demand {
    ($($variant:ident $({ $($field:ident: $field_type:ty),* $(,)? })?),* $(,)?) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub enum ExecutionDemand {
            $($variant $({ $($field: $field_type),* })?),*
        }

        #[cfg(test)]
        pub(crate) const EXECUTION_DEMAND_VARIANT_COUNT: usize =
            [$(stringify!($variant)),*].len();
    };
}

define_execution_demand! {
    Default,
    Outputs {
        outputs: Box<[GraphOutputRef]>,
        include_default_results: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub provenance: CompileProvenance,
    /// Number of plan-global logical values addressable by `ValueRef`.
    pub value_count: u32,
    pub operations: Box<[PlannedOperation]>,
    /// Values supplied without a producing operation in this plan.
    pub value_sources: Box<[PlanValueSource]>,
    pub value_dependencies: Box<[ValueDependency]>,
    pub root_region: StructuredControlRegion,
    pub effect_dependencies: Box<[EffectDependency]>,
    pub relational_subplans: Box<[RelationalSubplan]>,
    pub resources: Box<[CompiledResourceRequirement]>,
    pub results: Box<[PlanResult]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PlanValueSource {
    /// A value supplied by the event or function activation that enters this plan.
    ExternalInput(ValueRef),
    /// A value produced by structured control, such as a branch, loop, or call result.
    ControlProduced(ValueRef),
}

impl PlanValueSource {
    pub const fn value(self) -> ValueRef {
        match self {
            Self::ExternalInput(value) | Self::ControlProduced(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionPlanAbi {
    pub provenance: CompileProvenance,
    pub parameters: BTreeMap<FunctionParameterId, ValueRef>,
    pub results: BTreeMap<FunctionParameterId, ValueRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedOperation {
    pub source_node_id: NodeId,
    pub source_node_type_id: NodeTypeId,
    pub kernel: PlannedKernel,
    pub inputs: Box<[PlannedInput]>,
    pub outputs: Box<[PlannedOutput]>,
    pub params: CompiledParameterHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedInput {
    pub value: ValueRef,
    pub consumption: InputConsumption,
    /// Compiled literal or protocol default used only when no frame value is connected.
    pub bound_value: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedOutput {
    pub value: ValueRef,
    pub production: OutputProduction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedKernel {
    Native(KernelHandle),
    Relational(RelationalSubplanIndex),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueDependency {
    pub source: ValueRef,
    pub destination: ValueRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectDependency {
    pub before: OperationIndex,
    pub after: OperationIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StructuredControlRegion {
    Sequence(Box<[ControlStep]>),
    If {
        condition: ValueRef,
        then_region: Box<StructuredControlRegion>,
        else_region: Box<StructuredControlRegion>,
        results: Box<[BranchResultBinding]>,
    },
    Loop {
        body: Box<StructuredControlRegion>,
        carried: Box<[LoopCarriedBinding]>,
        continue_condition: ValueRef,
        max_iterations: u64,
    },
    Call {
        target: FunctionPlanHandle,
        arguments: Box<[CallArgumentBinding]>,
        results: Box<[CallResultBinding]>,
        mandatory: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlStep {
    Operation(OperationIndex),
    Region(Box<StructuredControlRegion>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallArgumentBinding {
    pub caller_source: ValueRef,
    pub callee_destination: ValueRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallResultBinding {
    pub callee_source: ValueRef,
    pub caller_destination: ValueRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchResultBinding {
    pub destination: ValueRef,
    pub then_source: ValueRef,
    pub else_source: ValueRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopCarriedBinding {
    pub body_input: ValueRef,
    pub initial_source: ValueRef,
    pub next_source: ValueRef,
    pub result: ValueRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationalSubplan {
    pub backend: RelationalBackendId,
    pub compiled_plan: CompiledRelationalPlan,
    /// Bridges required to materialize inputs consumed by this subplan.
    pub materialization_bridges: Box<[PlannedMaterializationBridge]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedMaterializationBridge {
    pub producer_fragment: RelationalFragmentId,
    pub consumer_fragment: RelationalFragmentId,
    pub producer_subplan: RelationalSubplanIndex,
    pub consumer_subplan: RelationalSubplanIndex,
    pub bridge: MaterializationBridge,
}

/// Backend-independent relational data consumed by a single relational runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledRelationalPlan {
    /// Stable provenance and deterministic compilation order for the merged island.
    pub fragment_order: Box<[RelationalFragmentId]>,
    pub operators: Box<[RelationalOperator]>,
    /// Every fragment's compiled root, used to bind graph outputs explicitly.
    pub fragment_roots: Box<[RelationalFragmentRoot]>,
    /// Exact materialization bridge bound to each boundary input operator.
    pub bridge_inputs: Box<[RelationalBridgeInput]>,
    /// Fragment values required by cross-island materialization bridges.
    pub requested_fragment_outputs: Box<[RelationalFragmentId]>,
    /// Backend outputs in the exact order of the owning `PlannedOperation::outputs`.
    pub roots: Box<[RelationalOperatorIndex]>,
    pub pushdown_hints: Box<[RelationalPushdownHint]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationalFragmentRoot {
    pub fragment: RelationalFragmentId,
    pub operator: RelationalOperatorIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationalBridgeInput {
    pub operator: RelationalOperatorIndex,
    pub bridge: PlannedMaterializationBridge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationalOperator {
    /// An input supplied across a materialization boundary.
    Input { name: Box<str> },
    Source {
        resource: ResourceId,
        relation: Box<str>,
    },
    Project {
        input: RelationalOperatorIndex,
        columns: Box<[RelationalProjection]>,
    },
    Filter {
        input: RelationalOperatorIndex,
        predicate: RelationalExpression,
    },
    Rename {
        input: RelationalOperatorIndex,
        columns: Box<[RelationalRename]>,
    },
    Limit {
        input: RelationalOperatorIndex,
        rows: u64,
    },
    Union {
        inputs: Box<[RelationalOperatorIndex]>,
        all: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationalProjection {
    pub name: Box<str>,
    pub expression: RelationalExpression,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationalRename {
    pub from: Box<str>,
    pub to: Box<str>,
}

/// Structured expressions keep backend compilers from receiving executable query text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationalExpression {
    Column(Box<str>),
    Literal(RelationalLiteral),
    Equal(Box<Self>, Box<Self>),
    NotEqual(Box<Self>, Box<Self>),
    LessThan(Box<Self>, Box<Self>),
    LessThanOrEqual(Box<Self>, Box<Self>),
    GreaterThan(Box<Self>, Box<Self>),
    GreaterThanOrEqual(Box<Self>, Box<Self>),
    And(Box<[Self]>),
    Or(Box<[Self]>),
    Not(Box<Self>),
    IsNull(Box<Self>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationalLiteral {
    Null,
    Boolean(bool),
    Integer(i64),
    Decimal(CanonicalDecimal),
    String(Box<str>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationalPushdownHint {
    Projection {
        source: RelationalOperatorIndex,
        columns: Box<[Box<str>]>,
    },
    Predicate {
        source: RelationalOperatorIndex,
        predicate: RelationalExpression,
    },
    Limit {
        source: RelationalOperatorIndex,
        rows: u64,
    },
}

#[derive(Default)]
struct SourceLineage {
    projection: Option<Vec<Box<str>>>,
    has_unbounded_projection: bool,
    predicates: Vec<RelationalExpression>,
    limits: BTreeSet<u64>,
}

struct LineageRequest {
    projection: Option<Vec<Box<str>>>,
    predicates: Vec<RelationalExpression>,
}

pub(crate) fn infer_relational_pushdown_hints(
    operators: &[RelationalOperator],
    roots: &[RelationalOperatorIndex],
) -> Vec<RelationalPushdownHint> {
    let mut lineage = BTreeMap::<RelationalOperatorIndex, SourceLineage>::new();
    let roots = roots.iter().copied().collect::<BTreeSet<_>>();
    let mut unhintable_sources = BTreeSet::new();
    for root in &roots {
        collect_opaque_descendant_sources(operators, *root, false, &mut unhintable_sources);
    }
    for root in roots {
        trace_relational_lineage(
            operators,
            root,
            LineageRequest {
                projection: None,
                predicates: Vec::new(),
            },
            &mut lineage,
        );
    }

    let mut hints = Vec::new();
    for (source, lineage) in lineage {
        let metadata_hintable = !unhintable_sources.contains(&source);
        if metadata_hintable
            && !lineage.has_unbounded_projection
            && let Some(columns) = lineage.projection
            && !columns.is_empty()
        {
            hints.push(RelationalPushdownHint::Projection {
                source,
                columns: columns.into_boxed_slice(),
            });
        }
        if metadata_hintable {
            hints.extend(
                lineage
                    .predicates
                    .into_iter()
                    .map(|predicate| RelationalPushdownHint::Predicate { source, predicate }),
            );
        }
        hints.extend(
            lineage
                .limits
                .into_iter()
                .map(|rows| RelationalPushdownHint::Limit { source, rows }),
        );
    }
    hints
}

fn collect_opaque_descendant_sources(
    operators: &[RelationalOperator],
    index: RelationalOperatorIndex,
    opaque: bool,
    sources: &mut BTreeSet<RelationalOperatorIndex>,
) {
    let Some(operator) = operators.get(index.index()) else {
        return;
    };
    match operator {
        RelationalOperator::Input { .. } => {}
        RelationalOperator::Source { .. } => {
            if opaque {
                sources.insert(index);
            }
        }
        RelationalOperator::Project { input, columns } => {
            collect_opaque_descendant_sources(
                operators,
                *input,
                opaque || direct_projection_mapping(columns).is_none(),
                sources,
            );
        }
        RelationalOperator::Filter { input, .. }
        | RelationalOperator::Rename { input, .. }
        | RelationalOperator::Limit { input, .. } => {
            collect_opaque_descendant_sources(operators, *input, opaque, sources);
        }
        RelationalOperator::Union { inputs, .. } => {
            for input in inputs {
                collect_opaque_descendant_sources(operators, *input, true, sources);
            }
        }
    }
}

fn trace_relational_lineage(
    operators: &[RelationalOperator],
    index: RelationalOperatorIndex,
    mut request: LineageRequest,
    lineage: &mut BTreeMap<RelationalOperatorIndex, SourceLineage>,
) {
    let Some(operator) = operators.get(index.index()) else {
        return;
    };
    match operator {
        RelationalOperator::Input { .. } | RelationalOperator::Union { .. } => {}
        RelationalOperator::Source { .. } => {
            let source = lineage.entry(index).or_default();
            match request.projection {
                Some(mut columns) if !source.has_unbounded_projection => {
                    for predicate in &request.predicates {
                        collect_expression_columns(predicate, &mut columns);
                    }
                    let projection = source.projection.get_or_insert_default();
                    for column in columns {
                        push_unique(projection, column);
                    }
                }
                Some(_) => {}
                None => source.has_unbounded_projection = true,
            }
            for predicate in request.predicates {
                if !source.predicates.contains(&predicate) {
                    source.predicates.push(predicate);
                }
            }
        }
        RelationalOperator::Project { input, columns } => {
            let Some((mapping, ordered_sources)) = direct_projection_mapping(columns) else {
                return;
            };
            request.projection = Some(match request.projection {
                Some(demanded) => demanded
                    .into_iter()
                    .filter_map(|name| mapping.get(&name).cloned())
                    .fold(Vec::new(), |mut columns, name| {
                        push_unique(&mut columns, name);
                        columns
                    }),
                None => ordered_sources,
            });
            request.predicates = request
                .predicates
                .into_iter()
                .map(|predicate| rewrite_expression_columns(predicate, &mapping))
                .collect();
            trace_relational_lineage(operators, *input, request, lineage);
        }
        RelationalOperator::Filter { input, predicate } => {
            if !request.predicates.contains(predicate) {
                request.predicates.push(predicate.clone());
            }
            trace_relational_lineage(operators, *input, request, lineage);
        }
        RelationalOperator::Rename { input, columns } => {
            let mapping = columns
                .iter()
                .map(|rename| (rename.to.clone(), rename.from.clone()))
                .collect::<BTreeMap<_, _>>();
            if let Some(projection) = request.projection.as_mut() {
                *projection = projection
                    .iter()
                    .map(|name| mapping.get(name).cloned().unwrap_or_else(|| name.clone()))
                    .fold(Vec::new(), |mut columns, name| {
                        push_unique(&mut columns, name);
                        columns
                    });
            }
            request.predicates = request
                .predicates
                .into_iter()
                .map(|predicate| rewrite_expression_columns(predicate, &mapping))
                .collect();
            trace_relational_lineage(operators, *input, request, lineage);
        }
        RelationalOperator::Limit { input, rows } => {
            trace_relational_lineage(operators, *input, request, lineage);
            if matches!(
                operators.get(input.index()),
                Some(RelationalOperator::Source { .. })
            ) {
                lineage.entry(*input).or_default().limits.insert(*rows);
            }
        }
    }
}

fn direct_projection_mapping(
    columns: &[RelationalProjection],
) -> Option<(BTreeMap<Box<str>, Box<str>>, Vec<Box<str>>)> {
    let mut mapping = BTreeMap::new();
    let mut ordered_sources = Vec::new();
    for projection in columns {
        let RelationalExpression::Column(source) = &projection.expression else {
            return None;
        };
        mapping.insert(projection.name.clone(), source.clone());
        push_unique(&mut ordered_sources, source.clone());
    }
    Some((mapping, ordered_sources))
}

fn push_unique(values: &mut Vec<Box<str>>, value: Box<str>) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn collect_expression_columns(expression: &RelationalExpression, columns: &mut Vec<Box<str>>) {
    match expression {
        RelationalExpression::Column(name) => push_unique(columns, name.clone()),
        RelationalExpression::Literal(_) => {}
        RelationalExpression::Equal(left, right)
        | RelationalExpression::NotEqual(left, right)
        | RelationalExpression::LessThan(left, right)
        | RelationalExpression::LessThanOrEqual(left, right)
        | RelationalExpression::GreaterThan(left, right)
        | RelationalExpression::GreaterThanOrEqual(left, right) => {
            collect_expression_columns(left, columns);
            collect_expression_columns(right, columns);
        }
        RelationalExpression::And(expressions) | RelationalExpression::Or(expressions) => {
            for expression in expressions {
                collect_expression_columns(expression, columns);
            }
        }
        RelationalExpression::Not(expression) | RelationalExpression::IsNull(expression) => {
            collect_expression_columns(expression, columns);
        }
    }
}

fn rewrite_expression_columns(
    expression: RelationalExpression,
    mapping: &BTreeMap<Box<str>, Box<str>>,
) -> RelationalExpression {
    let rewrite = |expression| Box::new(rewrite_expression_columns(expression, mapping));
    match expression {
        RelationalExpression::Column(name) => {
            RelationalExpression::Column(mapping.get(&name).cloned().unwrap_or(name))
        }
        RelationalExpression::Literal(value) => RelationalExpression::Literal(value),
        RelationalExpression::Equal(left, right) => {
            RelationalExpression::Equal(rewrite(*left), rewrite(*right))
        }
        RelationalExpression::NotEqual(left, right) => {
            RelationalExpression::NotEqual(rewrite(*left), rewrite(*right))
        }
        RelationalExpression::LessThan(left, right) => {
            RelationalExpression::LessThan(rewrite(*left), rewrite(*right))
        }
        RelationalExpression::LessThanOrEqual(left, right) => {
            RelationalExpression::LessThanOrEqual(rewrite(*left), rewrite(*right))
        }
        RelationalExpression::GreaterThan(left, right) => {
            RelationalExpression::GreaterThan(rewrite(*left), rewrite(*right))
        }
        RelationalExpression::GreaterThanOrEqual(left, right) => {
            RelationalExpression::GreaterThanOrEqual(rewrite(*left), rewrite(*right))
        }
        RelationalExpression::And(expressions) => RelationalExpression::And(
            expressions
                .into_vec()
                .into_iter()
                .map(|expression| rewrite_expression_columns(expression, mapping))
                .collect(),
        ),
        RelationalExpression::Or(expressions) => RelationalExpression::Or(
            expressions
                .into_vec()
                .into_iter()
                .map(|expression| rewrite_expression_columns(expression, mapping))
                .collect(),
        ),
        RelationalExpression::Not(expression) => RelationalExpression::Not(rewrite(*expression)),
        RelationalExpression::IsNull(expression) => {
            RelationalExpression::IsNull(rewrite(*expression))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaterializationBridge {
    Stream,
    Buffer,
    Collect,
    Spill,
    Replay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledResourceRequirement {
    pub resource: ResourceId,
    pub kind: ResourceKind,
    pub access: ResourceAccess,
    pub optional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceKind {
    DatabaseConnection,
    Accelerator,
    Sidecar,
    TemporaryStorage,
    ExternalArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceAccess {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanResult {
    pub name: Box<str>,
    pub output: GraphOutputRef,
    pub value: ValueRef,
}

#[cfg(test)]
mod task1_tests {
    use super::*;
    use crate::node_system::protocol::CanonicalDecimal;

    #[test]
    fn relational_decimal_literal_roundtrips_without_float_loss() {
        let literal = RelationalLiteral::Decimal(CanonicalDecimal::new("10.5").unwrap());

        let encoded = serde_json::to_value(&literal).unwrap();
        let decoded: RelationalLiteral = serde_json::from_value(encoded.clone()).unwrap();

        assert_eq!(decoded, literal);
        assert_eq!(encoded, serde_json::json!({"Decimal":"10.5"}));
        assert!(
            serde_json::from_value::<RelationalLiteral>(serde_json::json!({"Decimal":"1.0"}))
                .is_err()
        );
    }
}
