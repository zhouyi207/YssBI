use super::support::*;
use yss_node_protocol::*;

pub(super) fn register(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    register_view(fragment)?;
    Ok(())
}

fn register_view(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    const ID: &str = crate::DEBUG_VIEW_NODE_TYPE;
    let value_type = TypeParameterId::new("value").map_err(|source| {
        BuiltinAssemblyError::InvalidSemanticId {
            value: "value".into(),
            source,
        }
    })?;
    let mut input = data_port(
        "data",
        "Data",
        PortDirection::Input,
        TypeExpr::Generic(value_type.clone()),
        PortCardinality::Declared,
        LiteralPolicy::Allowed,
    )?;
    input.input_binding = Some(InputBindingSpec {
        literal_policy: LiteralPolicy::Forbidden,
        default_value: None,
    });
    fragment.nodes.push(leaf(
        protocol(ID, "debug", vec![input], vec![value_type], vec![], pure())?,
        ID,
    ));
    Ok(())
}
