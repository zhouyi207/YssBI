use super::identity::{
    KernelId, PlanFunctionParameterId, PlanInputGroupId, PlanNodeTypeId, PlanOutputRef,
    PlanPortAddress, PlanSourceIdentity,
};
use super::observation::{PlanObservationIntent, ValueRef};
use super::parameter::{CompiledParameterHandle, PlanParameterFieldId};
use super::result_category::ResultCategory;
use std::collections::BTreeMap;
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
    contract: PlanInputContract,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanInputContract {
    pub group: Option<PlanInputGroupId>,
    pub expected_type: DataType,
    pub coercions: Box<[PlanInputCoercionKind]>,
}

impl PlanInputBinding {
    pub fn new(
        port: PlanPortAddress,
        source: PlanInputSource,
        contract: PlanInputContract,
    ) -> Self {
        Self {
            port,
            source,
            contract,
        }
    }

    pub fn port(&self) -> &PlanPortAddress {
        &self.port
    }

    pub fn source(&self) -> &PlanInputSource {
        &self.source
    }

    pub fn contract(&self) -> &PlanInputContract {
        &self.contract
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanOperation {
    source: PlanSourceIdentity,
    node_type: PlanNodeTypeId,
    parameters: BTreeMap<PlanParameterFieldId, CompiledParameterHandle>,
    inputs: Box<[PlanInputBinding]>,
    observation_intents: Box<[PlanObservationIntent]>,
    outputs: Box<[PlanOutputBinding]>,
    specialization: PlanKernelSpecialization,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanKernelSpecialization {
    implementation: KernelId,
    input_types: Box<[PlanTypeBinding]>,
    output_types: Box<[PlanTypeBinding]>,
    coercions: Box<[PlanInputCoercion]>,
}

impl PlanKernelSpecialization {
    pub fn new(
        implementation: KernelId,
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

    pub fn implementation(&self) -> &KernelId {
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
    contract: PlanOutputContract,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanOutputContract {
    pub data_type: DataType,
    pub schema: Option<Box<[PlanOutputField]>>,
    pub category: ResultCategory,
    pub source: PlanSourceIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanOutputField {
    pub name: Box<str>,
    pub data_type: DataType,
    pub lineage: Option<PlanFieldLineage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanFieldLineage {
    pub source_identity: Box<str>,
    pub field_identity: Box<str>,
}

impl PlanOutputBinding {
    pub fn new(output: PlanOutputRef, value: ValueRef, contract: PlanOutputContract) -> Self {
        Self {
            output,
            value,
            contract,
        }
    }

    pub fn output(&self) -> &PlanOutputRef {
        &self.output
    }

    pub const fn value(&self) -> ValueRef {
        self.value
    }

    pub fn contract(&self) -> &PlanOutputContract {
        &self.contract
    }
}

impl PlanOperation {
    pub fn new(
        source: PlanSourceIdentity,
        node_type: PlanNodeTypeId,
        parameters: BTreeMap<PlanParameterFieldId, CompiledParameterHandle>,
        inputs: Box<[PlanInputBinding]>,
        observation_intents: Box<[PlanObservationIntent]>,
        outputs: Box<[PlanOutputBinding]>,
        specialization: PlanKernelSpecialization,
    ) -> Self {
        Self {
            source,
            node_type,
            parameters,
            inputs,
            observation_intents,
            outputs,
            specialization,
        }
    }

    pub fn source(&self) -> &PlanSourceIdentity {
        &self.source
    }

    pub fn node_type(&self) -> &PlanNodeTypeId {
        &self.node_type
    }

    pub fn kernel_id(&self) -> &KernelId {
        self.specialization.implementation()
    }

    pub fn parameters(&self) -> &BTreeMap<PlanParameterFieldId, CompiledParameterHandle> {
        &self.parameters
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
    parameters: Box<[FunctionPlanParameter]>,
    result: Option<FunctionPlanResult>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionPlanParameter {
    pub id: PlanFunctionParameterId,
    pub entry_output: PlanOutputRef,
    pub data_type: DataType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionPlanResult {
    pub id: PlanFunctionParameterId,
    pub return_input: PlanPortAddress,
    pub data_type: DataType,
}

impl FunctionPlanAbi {
    pub fn new(
        parameters: Box<[FunctionPlanParameter]>,
        result: Option<FunctionPlanResult>,
    ) -> Self {
        Self { parameters, result }
    }

    pub fn parameters(&self) -> &[FunctionPlanParameter] {
        &self.parameters
    }

    pub fn result(&self) -> Option<&FunctionPlanResult> {
        self.result.as_ref()
    }
}
