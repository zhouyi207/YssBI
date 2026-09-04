use std::collections::BTreeMap;

use yss_graph_analysis::{GraphKernelSpecialization, GraphResultCategory};
use yss_graph_analysis_contract::CompileId;
use yss_graph_document::{GraphResourcePath, NodeId, PortAddress};

/// Graph-owned value reference used while lowering a document.  Application
/// maps it to the execution package only after the Graph compilation result
/// has crossed the owner boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphValueRef(u32);

impl GraphValueRef {
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphInputSource {
    Value(GraphValueRef),
    Parameter(GraphParameterHandle),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphInputBinding {
    port: Box<str>,
    source: GraphInputSource,
}

impl GraphInputBinding {
    pub fn new(port: impl Into<Box<str>>, source: GraphInputSource) -> Self {
        Self {
            port: port.into(),
            source,
        }
    }

    pub fn port(&self) -> &str {
        &self.port
    }

    pub fn source(&self) -> &GraphInputSource {
        &self.source
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GraphParameterHandle(Box<str>);

impl GraphParameterHandle {
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GraphParameterValue {
    Scalar(GraphParameterScalar),
    Resource(Box<str>),
    List(Box<[GraphParameterValue]>),
    Record(BTreeMap<Box<str>, GraphParameterValue>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum GraphParameterScalar {
    Null,
    Bool(bool),
    Integer(i64),
    Unsigned(u64),
    Decimal(f64),
    String(Box<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphParameterPayload {
    schema: Box<str>,
    value: GraphParameterValue,
}

impl GraphParameterPayload {
    pub fn new(schema: impl Into<Box<str>>, value: GraphParameterValue) -> Self {
        Self {
            schema: schema.into(),
            value,
        }
    }

    pub fn schema(&self) -> &str {
        &self.schema
    }

    pub fn value(&self) -> &GraphParameterValue {
        &self.value
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphSourceIdentity {
    graph: GraphResourcePath,
    node: Option<NodeId>,
    port: Option<PortAddress>,
}

impl GraphSourceIdentity {
    pub fn new(graph: GraphResourcePath, node: Option<NodeId>, port: Option<PortAddress>) -> Self {
        Self { graph, node, port }
    }

    pub fn graph(&self) -> &GraphResourcePath {
        &self.graph
    }

    pub const fn node(&self) -> Option<NodeId> {
        self.node
    }

    pub fn port(&self) -> Option<&PortAddress> {
        self.port.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphObservationIntent {
    InspectInput { source: GraphInputSource },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphOutputBinding {
    port: Box<str>,
    value: GraphValueRef,
}

impl GraphOutputBinding {
    pub fn new(port: impl Into<Box<str>>, value: GraphValueRef) -> Self {
        Self {
            port: port.into(),
            value,
        }
    }

    pub fn port(&self) -> &str {
        &self.port
    }

    pub const fn value(&self) -> GraphValueRef {
        self.value
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphOperation {
    source: GraphSourceIdentity,
    result_category: GraphResultCategory,
    parameter_handles: Box<[GraphParameterHandle]>,
    inputs: Box<[GraphInputBinding]>,
    observation_intents: Box<[GraphObservationIntent]>,
    outputs: Box<[GraphOutputBinding]>,
    specialization: GraphKernelSpecialization,
}

impl GraphOperation {
    pub fn new(
        source: GraphSourceIdentity,
        result_category: GraphResultCategory,
        parameter_handles: Box<[GraphParameterHandle]>,
        inputs: Box<[GraphInputBinding]>,
        observation_intents: Box<[GraphObservationIntent]>,
        outputs: Box<[GraphOutputBinding]>,
        specialization: GraphKernelSpecialization,
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

    pub fn source(&self) -> &GraphSourceIdentity {
        &self.source
    }

    pub fn kind(&self) -> &str {
        &self.specialization.implementation
    }

    pub const fn result_category(&self) -> GraphResultCategory {
        self.result_category
    }

    pub fn parameter_handles(&self) -> &[GraphParameterHandle] {
        &self.parameter_handles
    }

    pub fn inputs(&self) -> &[GraphInputBinding] {
        &self.inputs
    }

    pub fn observation_intents(&self) -> &[GraphObservationIntent] {
        &self.observation_intents
    }

    pub fn outputs(&self) -> &[GraphOutputBinding] {
        &self.outputs
    }

    pub fn specialization(&self) -> &GraphKernelSpecialization {
        &self.specialization
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphCompiledPackage {
    graph: GraphResourcePath,
    compile_id: CompileId,
    operations: Box<[GraphOperation]>,
    parameters: BTreeMap<GraphParameterHandle, GraphParameterPayload>,
}

impl GraphCompiledPackage {
    pub fn new(
        graph: GraphResourcePath,
        compile_id: CompileId,
        operations: Box<[GraphOperation]>,
        parameters: BTreeMap<GraphParameterHandle, GraphParameterPayload>,
    ) -> Self {
        Self {
            graph,
            compile_id,
            operations,
            parameters,
        }
    }

    pub fn graph(&self) -> &GraphResourcePath {
        &self.graph
    }

    pub const fn compile_id(&self) -> CompileId {
        self.compile_id
    }

    pub fn operations(&self) -> &[GraphOperation] {
        &self.operations
    }

    pub fn parameters(&self) -> &BTreeMap<GraphParameterHandle, GraphParameterPayload> {
        &self.parameters
    }
}
