pub(crate) use crate::builtin::BuiltinAssemblyError;
pub(crate) use crate::builtin::ProviderFragment;
use crate::builtin::{assembled_interface, assembled_parameters, sid};
use crate::{Aliases, Text};
use std::collections::BTreeSet;
use std::sync::Arc;
use yss_graph_protocol::*;
use yss_graph_registry::{CategoryRegistration, RegisteredNode, TransparentNodeRole};

impl ProviderFragment {
    pub(crate) fn add_node_messages(
        &mut self,
        spec: &NodeTextSpec,
    ) -> Result<(), BuiltinAssemblyError> {
        let keys = NodeKeys::new(spec.id)?;
        for (locale, title, documentation, aliases) in [
            ("en-US", spec.title, spec.documentation, spec.aliases),
            (
                "zh-CN",
                spec.zh_title,
                spec.zh_documentation,
                spec.zh_aliases,
            ),
        ] {
            self.text(locale, keys.title.clone(), title);
            self.text(locale, keys.documentation.clone(), documentation);
            self.aliases(locale, keys.aliases.clone(), aliases);
        }
        Ok(())
    }

    pub(crate) fn text(&mut self, locale: &'static str, key: I18nKey, value: &'static str) {
        self.messages
            .push((locale, leak(key.as_str().to_owned()), Text(value)));
    }

    pub(crate) fn aliases(
        &mut self,
        locale: &'static str,
        key: I18nKey,
        values: &'static [&'static str],
    ) {
        self.messages
            .push((locale, leak(key.as_str().to_owned()), Aliases(values)));
    }
}

pub(crate) struct NodeTextSpec {
    pub id: &'static str,
    pub title: &'static str,
    pub zh_title: &'static str,
    pub documentation: &'static str,
    pub zh_documentation: &'static str,
    pub aliases: &'static [&'static str],
    pub zh_aliases: &'static [&'static str],
}

#[derive(Clone)]
struct NodeKeys {
    title: I18nKey,
    documentation: I18nKey,
    aliases: I18nKey,
}

impl NodeKeys {
    fn new(id: &'static str) -> Result<Self, BuiltinAssemblyError> {
        Ok(Self {
            title: i18n(leak(format!("nodes.{id}.title")))?,
            documentation: i18n(leak(format!("nodes.{id}.documentation")))?,
            aliases: i18n(leak(format!("nodes.{id}.aliases")))?,
        })
    }
}

pub(crate) fn leaf(protocol: NodeProtocol, kernel: &'static str) -> RegisteredNode {
    super::super::builtin::leaf(protocol, kernel)
}

pub(in crate::core_nodes) fn transparent(
    protocol: NodeProtocol,
    role: TransparentNodeRole,
) -> RegisteredNode {
    RegisteredNode::transparent(Arc::new(protocol), role)
}

pub(crate) fn protocol(
    id: &'static str,
    category: &'static str,
    ports: Vec<PortSpec>,
    type_parameters: Vec<TypeParameterId>,
    type_constraints: Vec<TypeConstraint>,
    parameters: Vec<ParameterSpec>,
    execution: ExecutionSemantics,
) -> Result<NodeProtocol, BuiltinAssemblyError> {
    let keys = NodeKeys::new(id)?;
    Ok(NodeProtocol {
        type_id: semantic(id, NodeTypeId::new)?,
        catalog: NodeCatalogProtocol {
            title_key: keys.title,
            documentation_key: Some(keys.documentation),
            aliases_key: Some(keys.aliases),
            category_id: semantic(category, NodeCategoryId::new)?,
            icon_id: semantic(leak(format!("builtin.{category}")), IconId::new)?,
            style_id: semantic("builtin.default", NodeStyleId::new)?,
            hidden: false,
        },
        interface: assembled_interface(id, ports, type_parameters, type_constraints, vec![])?,
        parameters: assembled_parameters(id, parameters)?,
        instance_display: NodeInstanceDisplaySpec::Static,
        execution,
        scope: NodeScope::Any,
        managed_role: None,
    })
}

pub(crate) fn data_port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port_with_instances(key, title, direction, value_type, PortInstances::Declared)
}

pub(crate) fn data_port_with_instances(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
    instances: PortInstances,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: semantic(key, PortKey::new)?,
        title: title.into(),
        direction,
        kind: PortKind::Data,
        value_type,
        instances,
        connections: ConnectionsPerPort::Single,
        input_binding: (direction == PortDirection::Input).then_some(InputBindingSpec {
            literal_policy: LiteralPolicy::Allowed,
            default_value: None,
        }),
        consumption: (direction == PortDirection::Input)
            .then_some(InputConsumption::FullyMaterialized),
        production: (direction == PortDirection::Output)
            .then_some(OutputProduction::FullyMaterialized),
        editor: PortEditorSpec::Default,
        schema: None,
    })
}

pub(crate) fn parameter(
    node_id: &'static str,
    key: &'static str,
    value_type: TypeExpr,
    default_value: Option<ParameterValue>,
    constraints: Vec<ParameterConstraint>,
    editor: ParameterEditorSpec,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    Ok(ParameterSpec {
        key: semantic(key, ParameterKey::new)?,
        title_key: parameter_key(node_id, key, "title")?,
        description_key: Some(parameter_key(node_id, key, "description")?),
        value_type,
        default_value,
        constraints,
        editor,
        presentation: ParameterPresentation::DetailPanel,
    })
}

pub(crate) fn concrete(id: &'static str) -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(TypeExpr::Concrete(semantic(id, TypeId::new)?))
}

pub(crate) fn data_series(element: &'static str) -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(TypeExpr::Applied {
        constructor: semantic("core.data_series", TypeConstructorId::new)?,
        arguments: vec![concrete(element)?],
    })
}

pub(crate) fn pure() -> ExecutionSemantics {
    ExecutionSemantics {
        determinism: Determinism::Deterministic,
        cache: CachePolicy::PerRun,
    }
}

pub(crate) fn parameter_key(
    node_id: &'static str,
    key: &'static str,
    suffix: &'static str,
) -> Result<I18nKey, BuiltinAssemblyError> {
    i18n(leak(format!("nodes.{node_id}.parameters.{key}.{suffix}")))
}

pub(crate) fn add_parameter_messages(
    fragment: &mut ProviderFragment,
    node_id: &'static str,
    entries: &[(
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
    )],
) -> Result<(), BuiltinAssemblyError> {
    for (key, en, zh, description, zh_description) in entries {
        let title_key = parameter_key(node_id, key, "title")?;
        let description_key = parameter_key(node_id, key, "description")?;
        fragment.text("en-US", title_key.clone(), en);
        fragment.text("zh-CN", title_key, zh);
        fragment.text("en-US", description_key.clone(), description);
        fragment.text("zh-CN", description_key, zh_description);
    }
    Ok(())
}

pub(crate) fn category(
    id: &'static str,
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

pub(crate) fn empty_classes() -> BTreeSet<TypeClassId> {
    BTreeSet::new()
}

pub(crate) fn i18n(value: &'static str) -> Result<I18nKey, BuiltinAssemblyError> {
    semantic(value, I18nKey::new)
}

pub(crate) fn semantic<T>(
    value: &'static str,
    make: impl FnOnce(&'static str) -> Result<T, InvalidSemanticId>,
) -> Result<T, crate::BuiltinAssemblyError> {
    sid(value, make)
}

pub(crate) fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}
