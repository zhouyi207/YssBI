use super::KernelRecorder;
use crate::node_system::analysis::ResourceVersionSet;
use crate::node_system::compiler::{
    GraphCompiler, LoweredKernel, LoweredNode, LoweringContext, LoweringError, NodeLowerer,
    ResourceSnapshot,
};
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentNode, GraphDocument, NodeId, NodePosition,
    PortAddress,
};
use crate::node_system::plan::{
    CompiledParameterHandle, ExecutionPlan, GraphOutputRef, KernelHandle, PlanResult,
    PlannedPublication,
};
use crate::node_system::protocol::{
    CachePolicy, ConnectionsPerPort, Determinism, EffectSemantics, EvaluationPolicy,
    ExecutionSemantics, I18nKey, IconId, InputBindingSpec, LiteralPolicy, NodeCatalogProtocol,
    NodeCategoryId, NodeInstanceDisplaySpec, NodeInterfaceProtocol, NodeProtocol, NodeScope,
    NodeStyleId, NodeTypeId, ParameterSchema, PortDirection, PortEditorSpec, PortInstances,
    PortKey, PortKind, PortSpec, Purity, TypeExpr, Value,
};
use crate::node_system::registry::{
    CategoryRegistration, I18nManifest, NodeRegistry, NodeRegistryBuilder, ProviderRegistration,
    RegisteredNode, TypeRegistration,
};
use crate::node_system::runtime::{
    Kernel, KernelContext, KernelError, KernelRegistry, RuntimeValue,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyResourceSnapshot;

impl ResourceSnapshot for EmptyResourceSnapshot {
    fn versions(&self) -> ResourceVersionSet {
        BTreeMap::new()
    }
}

pub struct TestProviderBuilder {
    nodes: Vec<TestNodeRegistration>,
    recorder: KernelRecorder,
}

struct TestNodeRegistration {
    protocol: NodeProtocol,
    kernel_handle: KernelHandle,
    parameter_handle: CompiledParameterHandle,
    kernel: Box<dyn Kernel>,
    output_keys: Vec<PortKey>,
}

impl Default for TestProviderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestProviderBuilder {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            recorder: KernelRecorder::default(),
        }
    }

    pub fn constant(&mut self, node_type: NodeTypeId, output: PortKey, value: Value) -> &mut Self {
        self.leaf(node_type, vec![], vec![output], ConstantKernel(value))
    }

    pub fn add(
        &mut self,
        node_type: NodeTypeId,
        left: PortKey,
        right: PortKey,
        output: PortKey,
    ) -> &mut Self {
        self.leaf(node_type, vec![left, right], vec![output], AddKernel)
    }

    pub fn failing(
        &mut self,
        node_type: NodeTypeId,
        output: PortKey,
        message: impl Into<Box<str>>,
    ) -> &mut Self {
        self.leaf(
            node_type,
            vec![],
            vec![output],
            FailingKernel(message.into()),
        )
    }

    pub fn cancelling(&mut self, node_type: NodeTypeId, output: PortKey) -> &mut Self {
        self.leaf(node_type, vec![], vec![output], CancellingKernel)
    }

    pub fn leaf(
        &mut self,
        node_type: NodeTypeId,
        inputs: Vec<PortKey>,
        outputs: Vec<PortKey>,
        kernel: impl Kernel + 'static,
    ) -> &mut Self {
        let kernel_name = format!("testing.kernel.{}", node_type.as_str());
        let parameter_name = format!("testing.params.{}", node_type.as_str());
        let kernel_handle = KernelHandle::new(kernel_name.clone()).expect("valid test kernel ID");
        let parameter_handle =
            CompiledParameterHandle::new(parameter_name).expect("valid test parameter ID");
        let protocol = protocol(node_type, &inputs, &outputs);
        let mut output_keys = outputs;
        output_keys.sort();
        self.nodes.push(TestNodeRegistration {
            protocol,
            kernel_handle,
            parameter_handle,
            kernel: Box::new(kernel),
            output_keys,
        });
        self
    }

    pub fn build(self) -> TestProvider {
        let mut provider = ProviderRegistration::new("yssbi.testing".parse().unwrap());
        provider.types = vec![TypeRegistration {
            id: crate::node_system::protocol::TypeId::new("core.int64").unwrap(),
            title_key: I18nKey::new("types.int64.title").unwrap(),
            classes: BTreeSet::new(),
        }]
        .into_boxed_slice();
        provider.categories = vec![CategoryRegistration {
            id: "testing".parse().unwrap(),
            title_key: "categories.testing.title".parse().unwrap(),
            parent: None,
            order: 0,
        }]
        .into_boxed_slice();

        let mut keys = BTreeSet::from([
            "types.int64.title".parse().unwrap(),
            "categories.testing.title".parse().unwrap(),
        ]);
        let mut registered = Vec::new();
        let mut kernels = KernelRegistry::new();
        let mut outputs = BTreeMap::new();
        for node in self.nodes {
            keys.insert(node.protocol.catalog.title_key.clone());
            let lowerer = FixedLowerer {
                kernel: node.kernel_handle.clone(),
                parameters: node.parameter_handle,
            };
            outputs.insert(node.protocol.type_id.clone(), node.output_keys);
            registered.push(RegisteredNode::leaf(
                Arc::new(node.protocol),
                Arc::new(crate::node_system::compiler::NodeImplementation::new(
                    lowerer,
                )),
            ));
            let recorded = self
                .recorder
                .wrap(node.kernel_handle.as_str(), DynamicKernel(node.kernel));
            kernels.register(node.kernel_handle, recorded).unwrap();
        }
        provider.i18n = I18nManifest { keys };
        provider.nodes = registered.into_boxed_slice();

        let mut builder = NodeRegistryBuilder::new();
        builder.register_provider(provider).unwrap();
        TestProvider {
            registry: builder.freeze().unwrap(),
            kernels,
            recorder: self.recorder,
            outputs,
            resources: EmptyResourceSnapshot,
        }
    }
}

pub struct TestProvider {
    registry: NodeRegistry,
    kernels: KernelRegistry,
    recorder: KernelRecorder,
    outputs: BTreeMap<NodeTypeId, Vec<PortKey>>,
    resources: EmptyResourceSnapshot,
}

impl TestProvider {
    pub fn registry(&self) -> &NodeRegistry {
        &self.registry
    }

    pub fn kernels(&self) -> &KernelRegistry {
        &self.kernels
    }

    pub fn recorder(&self) -> &KernelRecorder {
        &self.recorder
    }

    pub fn compile(&self, document: &GraphDocument) -> crate::node_system::compiler::CompileResult {
        GraphCompiler::new(&self.registry, &self.resources).compile(document)
    }

    #[track_caller]
    pub fn expose_result(
        &self,
        plan: &mut ExecutionPlan,
        node: &TestNode,
        port: &PortKey,
        name: impl Into<Box<str>>,
    ) {
        let keys = self
            .outputs
            .get(&node.node_type)
            .unwrap_or_else(|| panic!("unknown test node type '{}'", node.node_type));
        let output_index = keys
            .iter()
            .position(|key| key == port)
            .unwrap_or_else(|| panic!("'{}' is not an output of '{}'", port, node.node_type));
        let operation = plan
            .operations
            .iter()
            .find(|operation| operation.source_node_id == node.id)
            .unwrap_or_else(|| panic!("node '{}' was not lowered", node.id));
        let output = operation
            .outputs
            .get(output_index)
            .unwrap_or_else(|| panic!("lowered output order did not match protocol"));
        let name = name.into();
        let graph_output = GraphOutputRef {
            graph_path: plan.provenance.graph_path.clone(),
            port: PortAddress::declared(node.id, port.clone()),
        };
        plan.results = vec![PlanResult {
            name: name.clone(),
            output: graph_output.clone(),
            value: output.value,
        }]
        .into_boxed_slice();
        plan.publications = vec![PlannedPublication::GraphResult {
            name,
            output: graph_output,
            value: output.value,
        }]
        .into_boxed_slice();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestNode {
    id: NodeId,
    node_type: NodeTypeId,
}

#[derive(Debug, Default)]
pub struct TestGraphBuilder {
    document: GraphDocument,
    next_node: u128,
    next_connection: u128,
}

impl TestGraphBuilder {
    pub fn new() -> Self {
        Self {
            document: GraphDocument::default(),
            next_node: 1,
            next_connection: 1,
        }
    }

    pub fn add_node(&mut self, node_type: NodeTypeId) -> TestNode {
        let id = NodeId::from_uuid(stable_uuid(0x4e4f_4445, self.next_node));
        self.next_node += 1;
        self.document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: node_type.clone(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: BTreeMap::new(),
                user_label: None,
            },
        );
        TestNode { id, node_type }
    }

    pub fn connect(
        &mut self,
        output_node: &TestNode,
        output: &PortKey,
        input_node: &TestNode,
        input: &PortKey,
    ) -> &mut Self {
        let id = ConnectionId::from_uuid(stable_uuid(0x434f_4e4e, self.next_connection));
        self.next_connection += 1;
        self.document.connections.insert(
            id,
            DocumentConnection {
                id,
                output: PortAddress::declared(output_node.id, output.clone()),
                input: PortAddress::declared(input_node.id, input.clone()),
                order: None,
            },
        );
        self
    }

    pub fn build(self) -> GraphDocument {
        self.document
    }
}

fn stable_uuid(namespace: u128, ordinal: u128) -> Uuid {
    Uuid::from_u128((namespace << 96) | ordinal)
}

struct FixedLowerer {
    kernel: KernelHandle,
    parameters: CompiledParameterHandle,
}

impl NodeLowerer for FixedLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Ok(LoweredNode {
            kernel: LoweredKernel::Native(self.kernel.clone()),
            parameters: self.parameters.clone(),
        })
    }
}

struct DynamicKernel(Box<dyn Kernel>);

impl Kernel for DynamicKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        self.0.execute(context, inputs)
    }
}

struct ConstantKernel(Value);
impl Kernel for ConstantKernel {
    fn execute(
        &self,
        _: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        Ok(vec![RuntimeValue::Scalar(self.0.clone())])
    }
}

struct AddKernel;
impl Kernel for AddKernel {
    fn execute(
        &self,
        _: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        match inputs {
            [
                RuntimeValue::Scalar(Value::Integer(left)),
                RuntimeValue::Scalar(Value::Integer(right)),
            ] => Ok(vec![RuntimeValue::Scalar(Value::Integer(left + right))]),
            _ => Err(KernelError::new("add expects two scalar integers")),
        }
    }
}

struct FailingKernel(Box<str>);
impl Kernel for FailingKernel {
    fn execute(
        &self,
        _: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        Err(KernelError::new(self.0.clone()))
    }
}

struct CancellingKernel;
impl Kernel for CancellingKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        _: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        context.cancellation.cancel();
        Ok(vec![RuntimeValue::Scalar(Value::Null)])
    }
}

fn protocol(node_type: NodeTypeId, inputs: &[PortKey], outputs: &[PortKey]) -> NodeProtocol {
    let title_key = I18nKey::new(format!("nodes.{}.title", node_type.as_str())).unwrap();
    let ports = inputs
        .iter()
        .map(|key| port(key, PortDirection::Input))
        .chain(outputs.iter().map(|key| port(key, PortDirection::Output)))
        .collect();
    NodeProtocol {
        type_id: node_type,
        catalog: NodeCatalogProtocol {
            title_key,
            documentation_key: None,
            aliases_key: None,
            category_id: NodeCategoryId::new("testing").unwrap(),
            icon_id: IconId::new("testing").unwrap(),
            style_id: NodeStyleId::new("testing").unwrap(),
            hidden: false,
        },
        interface: NodeInterfaceProtocol::new(ports, vec![], vec![]).unwrap(),
        parameters: ParameterSchema::default(),
        instance_display: NodeInstanceDisplaySpec::Static,
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            purity: Purity::Pure,
            evaluation: EvaluationPolicy::DemandDriven,
            cache: CachePolicy::PerRun,
            effects: EffectSemantics::None,
            idempotent: false,
            retry: None,
        },
        scope: NodeScope::Any,
        managed_role: None,
    }
}

fn port(key: &PortKey, direction: PortDirection) -> PortSpec {
    PortSpec {
        key: key.clone(),
        title: key.as_str().into(),
        direction,
        kind: PortKind::Data,
        value_type: TypeExpr::Concrete(
            crate::node_system::protocol::TypeId::new("core.int64").unwrap(),
        ),
        instances: PortInstances::Declared,
        connections: if direction == PortDirection::Input {
            ConnectionsPerPort::Single
        } else {
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            }
        },
        input_binding: (direction == PortDirection::Input).then_some(InputBindingSpec {
            literal_policy: LiteralPolicy::Forbidden,
            default_value: None,
        }),
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    }
}
