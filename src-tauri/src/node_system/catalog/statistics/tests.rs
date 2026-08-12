use super::{LEGACY_NODE_IDS, NODES, build_provider_fragment};
use crate::node_system::protocol::{
    LiteralPolicy, NodeInterfaceProtocol, TypeExpr, TypeId, data_series_type,
    numeric_data_series_type,
};
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
fn ols_summary_accepts_numeric_data_series_and_rejects_string_series() {
    let fragment = build_provider_fragment().expect("statistics built-in fixture must assemble");
    let summary = fragment
        .nodes
        .iter()
        .find(|node| node.protocol().type_id.as_str() == "yssbi.statistics.ols.summary")
        .expect("OLS Summary protocol");
    let port_type = |key: &str| {
        summary
            .protocol()
            .interface
            .ports
            .iter()
            .find(|port| port.key.as_str() == key)
            .unwrap_or_else(|| panic!("OLS Summary must expose {key}"))
            .value_type
            .clone()
    };
    let series = data_series_type;
    let int = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
    let float = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    let string = TypeExpr::Concrete(TypeId::new("core.string").unwrap());
    let numeric_union = numeric_data_series_type();

    for key in ["response", "predictors"] {
        let target = port_type(key);
        assert!(crate::node_system::compiler::type_exprs_assignable(
            &series(int.clone()),
            &target,
            &[],
            &[],
        ));
        assert!(crate::node_system::compiler::type_exprs_assignable(
            &series(float.clone()),
            &target,
            &[],
            &[],
        ));
        assert!(crate::node_system::compiler::type_exprs_assignable(
            &numeric_union,
            &target,
            &[],
            &[],
        ));
        assert!(!crate::node_system::compiler::type_exprs_assignable(
            &series(string.clone()),
            &target,
            &[],
            &[],
        ));
    }
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
