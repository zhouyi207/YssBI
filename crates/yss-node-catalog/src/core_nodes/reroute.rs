use super::support::{
    BuiltinAssemblyError, ProviderFragment, data_port, protocol, pure, semantic, transparent,
};
use crate::{REROUTE_INPUT_PORT, REROUTE_NODE_TYPE, REROUTE_OUTPUT_PORT};
use yss_node_protocol::{
    LiteralPolicy, NodeStyleId, NodeTypingSpec, PortCardinality, PortDirection, PortKey, TypeExpr,
    TypeParameterId,
};
use yss_node_registry::{RegisteredNode, TransparentNodeRole};

pub(crate) fn register(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    crate::builtin::add_node_messages(
        &mut fragment.messages,
        REROUTE_NODE_TYPE,
        ["Reroute", "重路由"],
        Some([&[], &[]]),
    );
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

fn build_protocol() -> Result<yss_node_registry::RegisteredNode, BuiltinAssemblyError> {
    let node_type = REROUTE_NODE_TYPE;
    let input_key = semantic(REROUTE_INPUT_PORT, PortKey::new)?;
    let output_key = semantic(REROUTE_OUTPUT_PORT, PortKey::new)?;
    let generic = semantic("t", TypeParameterId::new)?;
    let value_type = TypeExpr::Generic(generic.clone());
    let mut reroute = protocol(
        node_type,
        "dataflow",
        vec![
            data_port(
                REROUTE_INPUT_PORT,
                "Input",
                PortDirection::Input,
                value_type.clone(),
                PortCardinality::Declared,
                LiteralPolicy::Forbidden,
            )?,
            data_port(
                REROUTE_OUTPUT_PORT,
                "Output",
                PortDirection::Output,
                value_type,
                PortCardinality::Declared,
                LiteralPolicy::Forbidden,
            )?,
        ],
        vec![generic],
        vec![],
        pure(),
    )?;
    reroute.catalog.hidden = true;
    reroute.catalog.style_id = semantic("builtin.reroute", NodeStyleId::new)?;
    reroute.typing = NodeTypingSpec::Identity {
        input: input_key,
        output: output_key,
    };
    Ok(transparent(reroute, TransparentNodeRole::Reroute))
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_node_registry::{NodeRegistryBuilder, ProviderRegistration};

    #[test]
    fn reroute_role_rejects_literal_sources_before_plan_preparation() {
        let mut protocol = build_protocol().unwrap().protocol().clone();
        protocol.type_id = "test.routing.node".parse().unwrap();
        let register = |protocol| {
            let mut builder = NodeRegistryBuilder::new();
            crate::register_builtin_nodes(&mut builder).unwrap();
            let mut provider = ProviderRegistration::new("test.routing".parse().unwrap());
            provider.nodes = vec![RegisteredNode::transparent(
                protocol,
                TransparentNodeRole::Reroute,
            )]
            .into_boxed_slice();
            builder.register_provider(provider).unwrap();
            builder.freeze()
        };
        assert!(
            register(protocol.clone().into()).is_ok(),
            "the role is independent of the built-in node ID"
        );
        protocol.interface.ports[0]
            .input_binding
            .as_mut()
            .unwrap()
            .literal_policy = LiteralPolicy::Allowed;
        let error = register(protocol.into()).unwrap_err();
        assert!(
            matches!(error, yss_node_registry::NodeRegistrationError::InvalidRegistry(
            yss_node_registry::RegistryValidationError::InvalidNode { node, reason }
        ) if node.as_str() == "test.routing.node" && reason.contains("one connected input"))
        );
    }
}
