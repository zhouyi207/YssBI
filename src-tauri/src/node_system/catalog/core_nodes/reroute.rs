use super::support::{
    BuiltinAssemblyError, NodeTextSpec, ProviderFragment, add_port_messages, effectful, protocol,
    pure, semantic, transparent,
};
use crate::node_system::catalog::{
    CONTROL_REROUTE_NODE_TYPE, DATA_REROUTE_NODE_TYPE, EFFECT_REROUTE_NODE_TYPE,
    REROUTE_INPUT_PORT, REROUTE_OUTPUT_PORT,
};
use crate::node_system::protocol::{
    ConnectionsPerPort, InputBindingSpec, InputConsumption, LiteralPolicy, NodeStyleId,
    OutputProduction, PortDirection, PortEditorSpec, PortInstances, PortKey, PortKind, PortSpec,
    SchemaExpr, TypeExpr, TypeParameterId,
};
use crate::node_system::registry::{RegisteredNode, TransparentNodeRole};

pub(crate) fn register(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for kind in [PortKind::Data, PortKind::Control, PortKind::Effect] {
        let node_type = node_type_for_kind(kind);
        fragment.add_node_messages(&NodeTextSpec {
            id: node_type,
            title: "Reroute",
            zh_title: "重路由",
            description: "Reroutes a connection without runtime behavior.",
            zh_description: "在不增加运行时行为的情况下重路由连接。",
            documentation: "Persistent compiler-transparent connection routing point.",
            zh_documentation: "持久化且对编译器透明的连接路由点。",
            aliases: &[],
            zh_aliases: &[],
        })?;
        add_port_messages(
            fragment,
            node_type,
            &[
                (REROUTE_INPUT_PORT, "Input", "输入"),
                (REROUTE_OUTPUT_PORT, "Output", "输出"),
            ],
        )?;
        fragment.nodes.push(build_protocol(kind)?);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::node_system) struct RerouteProtocolContract {
    pub(in crate::node_system) input_key: PortKey,
    pub(in crate::node_system) output_key: PortKey,
}

pub(in crate::node_system) fn validate_reroute_protocol_contract(
    registered: &RegisteredNode,
    kind: PortKind,
) -> Result<RerouteProtocolContract, &'static str> {
    let canonical = build_protocol(kind).map_err(|_| "canonical reroute protocol is invalid")?;
    if registered.transparent_role() != canonical.transparent_role()
        || registered.implementation().is_some()
        || registered.structural_role().is_some()
        || registered.protocol() != canonical.protocol()
    {
        return Err("reroute registration does not match the canonical protocol");
    }
    let [input, output] = registered.protocol().interface.ports.as_ref() else {
        unreachable!("canonical reroute protocol has exactly two ports");
    };
    Ok(RerouteProtocolContract {
        input_key: input.key.clone(),
        output_key: output.key.clone(),
    })
}

pub(in crate::node_system::catalog) const fn node_type_for_kind(kind: PortKind) -> &'static str {
    match kind {
        PortKind::Data => DATA_REROUTE_NODE_TYPE,
        PortKind::Control => CONTROL_REROUTE_NODE_TYPE,
        PortKind::Effect => EFFECT_REROUTE_NODE_TYPE,
    }
}

fn build_protocol(
    kind: PortKind,
) -> Result<crate::node_system::registry::RegisteredNode, BuiltinAssemblyError> {
    let node_type = node_type_for_kind(kind);
    let input_key = semantic(REROUTE_INPUT_PORT, PortKey::new)?;
    let generic = semantic("t", TypeParameterId::new)?;
    let value_type = match kind {
        PortKind::Data => TypeExpr::Generic(generic.clone()),
        PortKind::Control | PortKind::Effect => TypeExpr::Unknown,
    };
    let mut reroute = protocol(
        node_type,
        "control",
        vec![
            port(
                node_type,
                REROUTE_INPUT_PORT,
                PortDirection::Input,
                kind,
                value_type.clone(),
            )?,
            port(
                node_type,
                REROUTE_OUTPUT_PORT,
                PortDirection::Output,
                kind,
                value_type,
            )?,
        ],
        (kind == PortKind::Data)
            .then_some(generic)
            .into_iter()
            .collect(),
        vec![],
        vec![],
        if kind == PortKind::Effect {
            effectful()
        } else {
            pure()
        },
    )?;
    reroute.catalog.hidden = true;
    reroute.catalog.style_id = semantic("builtin.reroute", NodeStyleId::new)?;
    if kind == PortKind::Data {
        reroute.interface.ports[1].schema = Some(SchemaExpr::Input(input_key));
    }
    Ok(transparent(reroute, TransparentNodeRole::Reroute))
}

fn port(
    node_type: &'static str,
    key: &'static str,
    direction: PortDirection,
    kind: PortKind,
    value_type: TypeExpr,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: semantic(key, PortKey::new)?,
        label_key: super::support::port_key(node_type, key)?,
        direction,
        kind,
        value_type,
        instances: PortInstances::Declared,
        connections: if direction == PortDirection::Input {
            ConnectionsPerPort::Single
        } else {
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            }
        },
        input_binding: (direction == PortDirection::Input && kind == PortKind::Data).then_some(
            InputBindingSpec {
                literal_policy: LiteralPolicy::Forbidden,
                default_value: None,
            },
        ),
        consumption: (direction == PortDirection::Input && kind == PortKind::Data)
            .then_some(InputConsumption::FullyMaterialized),
        production: (direction == PortDirection::Output && kind == PortKind::Data)
            .then_some(OutputProduction::FullyMaterialized),
        editor: PortEditorSpec::Default,
        schema: None,
    })
}
