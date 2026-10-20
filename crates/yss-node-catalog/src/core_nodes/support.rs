use crate::builtin::node_key_text;
pub(crate) use crate::builtin::{BuiltinAssemblyError, ProviderFragment, leaf, pure};
use crate::builtin::{assembled_interface, assembled_parameters};
use std::sync::Arc;
use yss_node_protocol::*;
use yss_node_registry::{CategoryRegistration, RegisteredNode, TransparentNodeRole};

#[derive(Clone)]
struct NodeKeys {
    title: I18nKey,
    aliases: I18nKey,
}

impl NodeKeys {
    fn new(id: &str) -> Result<Self, BuiltinAssemblyError> {
        Ok(Self {
            title: i18n(node_key_text(id, "title"))?,
            aliases: i18n(node_key_text(id, "aliases"))?,
        })
    }
}

pub(in crate::core_nodes) fn transparent(
    protocol: NodeProtocol,
    role: TransparentNodeRole,
) -> RegisteredNode {
    RegisteredNode::transparent(Arc::new(protocol), role)
}

pub(crate) fn protocol(
    id: &str,
    category: &'static str,
    ports: Vec<PortSpec>,
    type_parameters: Vec<TypeParameterId>,
    parameters: Vec<Parameter>,
    execution: ExecutionSemantics,
) -> Result<NodeProtocol, BuiltinAssemblyError> {
    let keys = NodeKeys::new(id)?;
    Ok(NodeProtocol {
        type_id: semantic(id, NodeTypeId::new)?,
        catalog: NodeCatalogProtocol {
            title_key: keys.title,
            aliases_key: Some(keys.aliases),
            category_id: semantic(category, NodeCategoryId::new)?,
            icon_id: semantic(format!("builtin.{category}"), IconId::new)?,
            style_id: semantic("builtin.default", NodeStyleId::new)?,
            hidden: false,
        },
        interface: assembled_interface(id, ports, type_parameters, vec![])?,
        parameters: assembled_parameters(id, parameters)?,
        instance_display: NodeInstanceDisplaySpec::Static,
        execution,
        typing: NodeTypingSpec::Fixed,
        scope: NodeScope::Any,
        managed_role: None,
    })
}

pub(crate) fn parameter(
    node_id: &str,
    key: &'static str,
    value_type: TypeExpr,
    default_value: Option<TypedValue>,
    constraints: Vec<ParameterConstraint>,
    editor: ParameterEditorSpec,
) -> Result<Parameter, BuiltinAssemblyError> {
    Ok(Parameter {
        key: semantic(key, ParameterKey::new)?,
        title_key: parameter_key(node_id, key, "title")?,
        description_key: Some(parameter_key(node_id, key, "description")?),
        value_type,
        default_value,
        constraints,
        editor,
        presentation: ParameterPresentation::DetailPanel,
        visible_when: None,
    })
}

pub(crate) fn parameter_key(
    node_id: &str,
    key: &'static str,
    suffix: &'static str,
) -> Result<I18nKey, BuiltinAssemblyError> {
    i18n(format!("nodes.{node_id}.parameters.{key}.{suffix}"))
}

pub(crate) fn category(
    id: &str,
    title_key: &'static str,
    order: i32,
) -> Result<CategoryRegistration, BuiltinAssemblyError> {
    Ok(CategoryRegistration {
        id: semantic(id, NodeCategoryId::new)?,
        title_key: i18n(title_key)?,
        parent: None,
        order,
    })
}

pub(crate) use crate::builtin::{concrete, data_port, data_series, iid as i18n, sid as semantic};
