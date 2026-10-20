use super::builtin::{
    BuiltinAssemblyError, assembled_interface, assembled_parameters, iid, leaf, sid,
};
use crate::builtin::data_port;
use crate::builtin::node_key;
use std::sync::Arc;
use yss_node_protocol::*;
use yss_node_registry::{
    FUNCTION_CALL_ARGUMENTS_RESOLVER, FUNCTION_CALL_RESULTS_RESOLVER,
    FUNCTION_ENTRY_PARAMETERS_RESOLVER, FUNCTION_RETURN_RESULTS_RESOLVER, RegisteredNode,
    StructuralNodeRole,
};

pub(crate) fn builtin_function_interface_resolver_ids() -> Box<[InterfaceResolverId]> {
    [
        FUNCTION_CALL_ARGUMENTS_RESOLVER,
        FUNCTION_CALL_RESULTS_RESOLVER,
        FUNCTION_ENTRY_PARAMETERS_RESOLVER,
        FUNCTION_RETURN_RESULTS_RESOLVER,
    ]
    .map(|value| InterfaceResolverId::new(value).expect("built-in resolver ID is valid"))
    .into()
}

pub(super) fn register(nodes: &mut Vec<RegisteredNode>) -> Result<(), BuiltinAssemblyError> {
    nodes.extend([
        RegisteredNode::structural(
            Arc::new(function_entry_protocol()?),
            StructuralNodeRole::FunctionEntry,
        ),
        RegisteredNode::structural(
            Arc::new(function_return_protocol()?),
            StructuralNodeRole::FunctionReturn,
        ),
        RegisteredNode::structural(
            Arc::new(function_call_protocol()?),
            StructuralNodeRole::Call,
        ),
        leaf(constant_get_protocol()?, "constant.get"),
    ]);
    Ok(())
}

fn function_entry_protocol() -> Result<NodeProtocol, BuiltinAssemblyError> {
    protocol(
        crate::FUNCTION_ENTRY_NODE_TYPE,
        vec![derived_data_port(
            "parameters",
            "Parameters",
            PortDirection::Output,
            FUNCTION_ENTRY_PARAMETERS_RESOLVER,
        )?],
        vec![],
        vec![resource_parameter("function")?],
        NodeScope::Function,
        Some(ManagedNodeRole::FunctionEntry),
        environment_dependent(),
    )
}

fn function_return_protocol() -> Result<NodeProtocol, BuiltinAssemblyError> {
    protocol(
        crate::FUNCTION_RETURN_NODE_TYPE,
        vec![derived_data_port(
            "results",
            "Results",
            PortDirection::Input,
            FUNCTION_RETURN_RESULTS_RESOLVER,
        )?],
        vec![],
        vec![resource_parameter("function")?],
        NodeScope::Function,
        Some(ManagedNodeRole::FunctionReturn),
        environment_dependent(),
    )
}

fn function_call_protocol() -> Result<NodeProtocol, BuiltinAssemblyError> {
    protocol(
        crate::FUNCTION_CALL_NODE_TYPE,
        vec![
            derived_data_port(
                "arguments",
                "Arguments",
                PortDirection::Input,
                FUNCTION_CALL_ARGUMENTS_RESOLVER,
            )?,
            derived_data_port(
                "results",
                "Results",
                PortDirection::Output,
                FUNCTION_CALL_RESULTS_RESOLVER,
            )?,
        ],
        vec![],
        vec![resource_parameter("target")?],
        NodeScope::Any,
        None,
        environment_dependent(),
    )
}

fn constant_get_protocol() -> Result<NodeProtocol, BuiltinAssemblyError> {
    let generic = sid("value", TypeParameterId::new)?;
    let mut protocol = protocol(
        crate::CONSTANT_GET_NODE_TYPE,
        vec![data_port(
            "value",
            "Value",
            PortDirection::Output,
            TypeExpr::Generic(generic.clone()),
            PortCardinality::Declared,
            LiteralPolicy::Forbidden,
        )?],
        vec![generic],
        vec![Parameter {
            key: sid("constant", ParameterKey::new)?,
            title_key: iid("parameters.constant.title")?,
            description_key: Some(iid("parameters.constant.description")?),
            value_type: TypeExpr::Concrete(sid("core.text", TypeId::new)?),
            default_value: None,
            constraints: vec![ParameterConstraint::Required],
            editor: ParameterEditorSpec::GraphConstant,
            presentation: ParameterPresentation::DetailPanel,
            visible_when: None,
        }],
        NodeScope::Any,
        None,
        environment_dependent(),
    )?;
    protocol.catalog.category_id = sid("constants", NodeCategoryId::new)?;
    protocol.catalog.icon_id = sid("builtin.constants", IconId::new)?;
    protocol.execution.determinism = Determinism::Deterministic;
    protocol.typing = NodeTypingSpec::ConstantOutput {
        parameter: sid("constant", ParameterKey::new)?,
        output: sid("value", PortKey::new)?,
    };
    Ok(protocol)
}

fn protocol(
    id: &'static str,
    ports: Vec<PortSpec>,
    type_parameters: Vec<TypeParameterId>,
    parameters: Vec<Parameter>,
    scope: NodeScope,
    managed_role: Option<ManagedNodeRole>,
    execution: ExecutionSemantics,
) -> Result<NodeProtocol, BuiltinAssemblyError> {
    Ok(NodeProtocol {
        type_id: sid(id, NodeTypeId::new)?,
        catalog: NodeCatalogProtocol {
            title_key: node_key(id, "title")?,
            aliases_key: Some(node_key(id, "aliases")?),
            category_id: sid("project", NodeCategoryId::new)?,
            icon_id: sid("builtin.project", IconId::new)?,
            style_id: sid("builtin.default", NodeStyleId::new)?,
            hidden: false,
        },
        interface: assembled_interface(id, ports, type_parameters, vec![])?,
        parameters: assembled_parameters(id, parameters)?,
        instance_display: match id {
            crate::FUNCTION_CALL_NODE_TYPE => NodeInstanceDisplaySpec::ResourceParameter {
                parameter: sid("target", ParameterKey::new)?,
                kind: ResourceDisplayKind::Function,
            },
            _ => NodeInstanceDisplaySpec::Static,
        },
        execution,
        typing: NodeTypingSpec::Fixed,
        scope,
        managed_role,
    })
}

fn derived_data_port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
    resolver: &'static str,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port(
        key,
        title,
        direction,
        TypeExpr::Unknown,
        PortCardinality::Derived {
            resolver: sid(resolver, InterfaceResolverId::new)?,
        },
        LiteralPolicy::Forbidden,
    )
}

pub(crate) fn resource_parameter(key: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    let kind = match key {
        "target" | "function" => ResourceDisplayKind::Function,
        _ => {
            return Err(BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
                context: "project resource parameter",
                value: key.into(),
            });
        }
    };
    Ok(Parameter {
        key: sid(key, ParameterKey::new)?,
        title_key: iid(format!("parameters.{key}.title"))?,
        description_key: Some(iid(format!("parameters.{key}.description"))?),
        value_type: TypeExpr::Concrete(sid("core.text", TypeId::new)?),
        default_value: None,
        constraints: vec![ParameterConstraint::Required],
        editor: ParameterEditorSpec::Resource { kind },
        presentation: ParameterPresentation::DetailPanel,
        visible_when: None,
    })
}

fn environment_dependent() -> ExecutionSemantics {
    ExecutionSemantics {
        determinism: Determinism::EnvironmentDependent,
    }
}
