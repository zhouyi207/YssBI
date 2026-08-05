use super::{LEGACY_NODE_IDS, NODES, build_provider_fragment};
use crate::node_system::protocol::{LiteralPolicy, NodeInterfaceProtocol};
use std::collections::BTreeSet;

#[test]
fn every_legacy_statistics_node_has_one_stable_id() {
    assert_eq!(LEGACY_NODE_IDS.len(), 42);
    assert_eq!(NODES.len(), LEGACY_NODE_IDS.len());

    let legacy = LEGACY_NODE_IDS
        .iter()
        .map(|(name, _)| *name)
        .collect::<BTreeSet<_>>();
    let ids = LEGACY_NODE_IDS
        .iter()
        .map(|(_, id)| *id)
        .collect::<BTreeSet<_>>();

    assert_eq!(legacy.len(), LEGACY_NODE_IDS.len());
    assert_eq!(ids.len(), LEGACY_NODE_IDS.len());
    assert!(ids.iter().all(|id| id.starts_with("yssbi.statistics.")));
    for spec in NODES {
        assert_eq!(
            LEGACY_NODE_IDS
                .iter()
                .find(|(legacy_name, _)| *legacy_name == spec.legacy_name)
                .map(|(_, id)| *id),
            Some(spec.id),
        );
    }
}

#[test]
fn statistics_fragment_contains_every_migrated_protocol() {
    let fragment = build_provider_fragment().expect("statistics built-in fixture must assemble");
    assert_eq!(fragment.nodes.len(), LEGACY_NODE_IDS.len());
}

#[test]
fn statistics_protocols_have_unique_ports_and_valid_bindings() {
    for node in build_provider_fragment()
        .expect("statistics built-in fixture must assemble")
        .nodes
    {
        let protocol = &node.protocol();
        let keys = protocol
            .interface
            .ports
            .iter()
            .map(|port| &port.key)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            keys.len(),
            protocol.interface.ports.len(),
            "{}",
            protocol.type_id
        );

        NodeInterfaceProtocol::new(
            protocol.interface.ports.to_vec(),
            protocol.interface.type_parameters.to_vec(),
            protocol.interface.type_constraints.to_vec(),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", protocol.type_id));

        for port in &protocol.interface.ports {
            if let Some(default) = port
                .input_binding
                .as_ref()
                .and_then(|binding| binding.default_value.as_ref())
            {
                assert_eq!(
                    port.input_binding.as_ref().unwrap().literal_policy,
                    LiteralPolicy::Allowed,
                    "{}:{}",
                    protocol.type_id,
                    port.key
                );
                assert_eq!(
                    default.value_type, port.value_type,
                    "{}:{}",
                    protocol.type_id, port.key
                );
            }
        }
    }
}
