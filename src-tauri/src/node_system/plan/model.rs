use crate::node_system::analysis::CompileProvenance;
use crate::node_system::document::{FunctionParameterId, GraphResourcePath, NodeId, PortAddress};
use crate::node_system::protocol::{InputConsumption, NodeTypeId, OutputProduction};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionDemand {
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
    String(Box<str>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationalPushdownHint {
    Projection {
        source: RelationalOperatorIndex,
        columns: Box<[Box<str>]>,
    },
    Limit {
        source: RelationalOperatorIndex,
        rows: u64,
    },
}

pub(crate) fn infer_relational_pushdown_hints(
    operators: &[RelationalOperator],
) -> Vec<RelationalPushdownHint> {
    let mut hints = Vec::new();
    for operator in operators {
        match operator {
            RelationalOperator::Project { input, columns }
                if matches!(
                    operators.get(input.index()),
                    Some(RelationalOperator::Source { .. })
                ) && columns.iter().all(|column| {
                    matches!(&column.expression, RelationalExpression::Column(_))
                }) =>
            {
                let columns = columns
                    .iter()
                    .filter_map(|column| match &column.expression {
                        RelationalExpression::Column(name) => Some(name.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                hints.push(RelationalPushdownHint::Projection {
                    source: *input,
                    columns: columns.into_boxed_slice(),
                });
            }
            RelationalOperator::Limit { input, rows }
                if matches!(
                    operators.get(input.index()),
                    Some(RelationalOperator::Source { .. })
                ) =>
            {
                hints.push(RelationalPushdownHint::Limit {
                    source: *input,
                    rows: *rows,
                });
            }
            _ => {}
        }
    }
    hints
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
