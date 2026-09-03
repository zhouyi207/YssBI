use super::identity::{PlanOperationKind, PlanOutputRef, PlanPortAddress, PlanSourceIdentity};
use super::observation::{PlanObservationIntent, ValueRef};
use super::parameter::CompiledParameterHandle;
use super::result_category::ResultCategory;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanInputSource {
    Value(ValueRef),
    Parameter(CompiledParameterHandle),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanInputBinding {
    port: PlanPortAddress,
    source: PlanInputSource,
}

impl PlanInputBinding {
    pub fn new(port: PlanPortAddress, source: PlanInputSource) -> Self {
        Self { port, source }
    }

    pub fn port(&self) -> &PlanPortAddress {
        &self.port
    }

    pub fn source(&self) -> &PlanInputSource {
        &self.source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanOperation {
    source: PlanSourceIdentity,
    kind: PlanOperationKind,
    result_category: ResultCategory,
    parameter_handles: Box<[CompiledParameterHandle]>,
    inputs: Box<[PlanInputBinding]>,
    observation_intents: Box<[PlanObservationIntent]>,
    outputs: Box<[PlanOutputBinding]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanOutputBinding {
    output: PlanOutputRef,
    value: ValueRef,
}

impl PlanOutputBinding {
    pub fn new(output: PlanOutputRef, value: ValueRef) -> Self {
        Self { output, value }
    }

    pub fn output(&self) -> &PlanOutputRef {
        &self.output
    }

    pub const fn value(&self) -> ValueRef {
        self.value
    }
}

impl PlanOperation {
    pub fn new(
        source: PlanSourceIdentity,
        kind: PlanOperationKind,
        result_category: ResultCategory,
        parameter_handles: Box<[CompiledParameterHandle]>,
        inputs: Box<[PlanInputBinding]>,
        observation_intents: Box<[PlanObservationIntent]>,
        outputs: Box<[PlanOutputBinding]>,
    ) -> Self {
        Self {
            source,
            kind,
            result_category,
            parameter_handles,
            inputs,
            observation_intents,
            outputs,
        }
    }

    pub fn source(&self) -> &PlanSourceIdentity {
        &self.source
    }

    pub fn kind(&self) -> &PlanOperationKind {
        &self.kind
    }

    pub const fn result_category(&self) -> ResultCategory {
        self.result_category
    }

    pub fn parameter_handles(&self) -> &[CompiledParameterHandle] {
        &self.parameter_handles
    }

    pub fn inputs(&self) -> &[PlanInputBinding] {
        &self.inputs
    }

    pub fn observation_intents(&self) -> &[PlanObservationIntent] {
        &self.observation_intents
    }

    pub fn outputs(&self) -> &[PlanOutputBinding] {
        &self.outputs
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanExecutionDemand {
    Default,
    Outputs {
        outputs: Box<[PlanOutputRef]>,
        include_default_results: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionPlan {
    operations: Box<[PlanOperation]>,
}

impl ExecutionPlan {
    pub fn new(operations: Box<[PlanOperation]>) -> Self {
        Self { operations }
    }

    pub fn empty() -> Self {
        Self {
            operations: Box::new([]),
        }
    }

    pub fn operations(&self) -> &[PlanOperation] {
        &self.operations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionPlanAbi {
    signature: Box<str>,
}

impl FunctionPlanAbi {
    pub fn new(signature: Box<str>) -> Self {
        Self { signature }
    }

    pub fn signature(&self) -> &str {
        &self.signature
    }
}
