use crate::node_system::protocol::{
    CachePolicy, Determinism, EffectSemantics, EvaluationPolicy, ExecutionSemantics, I18nKey,
    IconId, ManagedNodeRole, NodeCatalogProtocol, NodeCategoryId, NodeInstanceDisplaySpec,
    NodeInterfaceProtocol, NodeProtocol, NodeScope, NodeStyleId, NodeTypeId, ParameterSchema,
    ParameterSpec, PortSpec, Purity,
};

pub(crate) struct TestProtocolBuilder {
    type_id: NodeTypeId,
    catalog: NodeCatalogProtocol,
    ports: Vec<PortSpec>,
    parameters: Vec<ParameterSpec>,
    execution: ExecutionSemantics,
    scope: NodeScope,
    managed_role: Option<ManagedNodeRole>,
}

impl TestProtocolBuilder {
    pub(crate) fn new(type_id: &str, category_id: &str) -> Self {
        let catalog_key = type_id.strip_prefix("yssbi.").unwrap_or(type_id);
        Self {
            type_id: NodeTypeId::new(type_id).expect("test node type ID is valid"),
            catalog: NodeCatalogProtocol {
                title_key: I18nKey::new(format!("nodes.{catalog_key}.title"))
                    .expect("test title key is valid"),
                description_key: None,
                documentation_key: None,
                aliases_key: None,
                category_id: NodeCategoryId::new(category_id).expect("test category ID is valid"),
                icon_id: IconId::new(category_id).expect("test icon ID is valid"),
                style_id: NodeStyleId::new("default").expect("test style ID is valid"),
                hidden: false,
            },
            ports: Vec::new(),
            parameters: Vec::new(),
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

    pub(crate) fn style(mut self, style_id: &str) -> Self {
        self.catalog.style_id = NodeStyleId::new(style_id).expect("test style ID is valid");
        self
    }

    pub(crate) fn ports(mut self, ports: Vec<PortSpec>) -> Self {
        self.ports = ports;
        self
    }

    pub(crate) fn parameters(mut self, parameters: Vec<ParameterSpec>) -> Self {
        self.parameters = parameters;
        self
    }

    pub(crate) fn execution(mut self, execution: ExecutionSemantics) -> Self {
        self.execution = execution;
        self
    }

    pub(crate) fn scope(mut self, scope: NodeScope) -> Self {
        self.scope = scope;
        self
    }

    pub(crate) fn managed_role(mut self, role: Option<ManagedNodeRole>) -> Self {
        self.managed_role = role;
        self
    }

    pub(crate) fn build(self) -> NodeProtocol {
        crate::node_system::protocol::validate_execution(self.execution)
            .expect("test execution semantics are valid");
        NodeProtocol {
            type_id: self.type_id,
            catalog: self.catalog,
            interface: NodeInterfaceProtocol::new(self.ports, Vec::new(), Vec::new())
                .expect("test port contracts are valid"),
            parameters: ParameterSchema::new(self.parameters)
                .expect("test parameter schema is valid"),
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: self.execution,
            scope: self.scope,
            managed_role: self.managed_role,
        }
    }
}
