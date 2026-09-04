use super::support::{
    BuiltinAssemblyError, NodeTextSpec, ProviderFragment, protocol, pure, semantic, transparent,
};
use crate::{REROUTE_INPUT_PORT, REROUTE_NODE_TYPE, REROUTE_OUTPUT_PORT};
use yss_graph_protocol::{
    ConnectionsPerPort, InputBindingSpec, InputConsumption, LiteralPolicy, NodeStyleId,
    OutputProduction, PortDirection, PortEditorSpec, PortInstances, PortKey, PortSpec, SchemaExpr,
    TypeExpr, TypeParameterId,
};
use yss_graph_registry::{RegisteredNode, TransparentNodeRole};

pub(crate) fn register(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    fragment.add_node_messages(&NodeTextSpec {
        id: REROUTE_NODE_TYPE,
        title: "Reroute",
        zh_title: "重路由",
        documentation: "Persistent compiler-transparent data routing point.",
        zh_documentation: "持久化且对编译器透明的数据路由点。",
        aliases: &[],
        zh_aliases: &[],
    })?;
    fragment.nodes.push(build_protocol()?);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RerouteProtocolContract {
    pub input_key: PortKey,
    pub output_key: PortKey,
}

pub fn validate_reroute_protocol_contract(
    registered: &RegisteredNode,
) -> Result<RerouteProtocolContract, &'static str> {
    let canonical = build_protocol().map_err(|_| "canonical reroute protocol is invalid")?;
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

fn build_protocol() -> Result<yss_graph_registry::RegisteredNode, BuiltinAssemblyError> {
    let node_type = REROUTE_NODE_TYPE;
    let input_key = semantic(REROUTE_INPUT_PORT, PortKey::new)?;
    let generic = semantic("t", TypeParameterId::new)?;
    let value_type = TypeExpr::Generic(generic.clone());
    let mut reroute = protocol(
        node_type,
        "dataflow",
        vec![
            port(
                REROUTE_INPUT_PORT,
                "Input",
                PortDirection::Input,
                value_type.clone(),
            )?,
            port(
                REROUTE_OUTPUT_PORT,
                "Output",
                PortDirection::Output,
                value_type,
            )?,
        ],
        vec![generic],
        vec![],
        vec![],
        pure(),
    )?;
    reroute.catalog.hidden = true;
    reroute.catalog.style_id = semantic("builtin.reroute", NodeStyleId::new)?;
    reroute.interface.ports[1].schema = Some(SchemaExpr::Input(input_key));
    Ok(transparent(reroute, TransparentNodeRole::Reroute))
}

fn port(
    key: &'static str,
    title: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: semantic(key, PortKey::new)?,
        title: title.into(),
        direction,
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
        input_binding: (direction == PortDirection::Input).then_some(InputBindingSpec {
            literal_policy: LiteralPolicy::Forbidden,
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
