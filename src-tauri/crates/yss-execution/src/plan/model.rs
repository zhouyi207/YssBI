use super::identity::{PlanOperationKind, PlanOutputRef, PlanPortAddress, PlanSourceIdentity};
use super::observation::{PlanObservationIntent, ValueRef};
use super::parameter::CompiledParameterHandle;
use super::result_category::ResultCategory;
use yss_data_contract::DataType;

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
    result_category: ResultCategory,
    parameter_handles: Box<[CompiledParameterHandle]>,
    inputs: Box<[PlanInputBinding]>,
    observation_intents: Box<[PlanObservationIntent]>,
    outputs: Box<[PlanOutputBinding]>,
    specialization: PlanKernelSpecialization,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanKernelSpecialization {
    implementation: PlanOperationKind,
    input_types: Box<[PlanTypeBinding]>,
    output_types: Box<[PlanTypeBinding]>,
    coercions: Box<[PlanInputCoercion]>,
}

impl PlanKernelSpecialization {
    pub fn new(
        implementation: PlanOperationKind,
        input_types: Box<[PlanTypeBinding]>,
        output_types: Box<[PlanTypeBinding]>,
        coercions: Box<[PlanInputCoercion]>,
    ) -> Self {
        Self {
            implementation,
            input_types,
            output_types,
            coercions,
        }
    }

    pub fn implementation(&self) -> &PlanOperationKind {
        &self.implementation
    }

    pub fn input_types(&self) -> &[PlanTypeBinding] {
        &self.input_types
    }

    pub fn output_types(&self) -> &[PlanTypeBinding] {
        &self.output_types
    }

    pub fn coercions(&self) -> &[PlanInputCoercion] {
        &self.coercions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanTypeBinding {
    port: PlanPortAddress,
    data_type: DataType,
}

impl PlanTypeBinding {
    pub fn new(port: PlanPortAddress, data_type: DataType) -> Self {
        Self { port, data_type }
    }

    pub fn port(&self) -> &PlanPortAddress {
        &self.port
    }

    pub fn data_type(&self) -> &DataType {
        &self.data_type
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanInputCoercionKind {
    WidenInt64ToFloat64,
    BroadcastScalarToSeries,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanInputCoercion {
    port: PlanPortAddress,
    kind: PlanInputCoercionKind,
}

impl PlanInputCoercion {
    pub fn new(port: PlanPortAddress, kind: PlanInputCoercionKind) -> Self {
        Self { port, kind }
    }

    pub fn port(&self) -> &PlanPortAddress {
        &self.port
    }

    pub const fn kind(&self) -> PlanInputCoercionKind {
        self.kind
    }
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
        result_category: ResultCategory,
        parameter_handles: Box<[CompiledParameterHandle]>,
        inputs: Box<[PlanInputBinding]>,
        observation_intents: Box<[PlanObservationIntent]>,
        outputs: Box<[PlanOutputBinding]>,
        specialization: PlanKernelSpecialization,
    ) -> Self {
        Self {
            source,
            result_category,
            parameter_handles,
            inputs,
            observation_intents,
            outputs,
            specialization,
        }
    }

    pub fn source(&self) -> &PlanSourceIdentity {
        &self.source
    }

    pub fn kind(&self) -> &PlanOperationKind {
        self.specialization.implementation()
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

    pub fn specialization(&self) -> &PlanKernelSpecialization {
        &self.specialization
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
