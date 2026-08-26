use super::build_provider_fragment;
use crate::graph_document::{NodeId, PortAddress};
use crate::node_system::catalog::localization::Message;
use crate::node_system::compiler::{
    CompileCancellationToken, LoweredKernel, LoweringContext, NodeImplementation,
    ValidatedNodeConfig,
};
use crate::node_system::plan::ValueRef;
use crate::node_system::protocol::{PortDirection, PortKind};
use std::collections::BTreeSet;

#[test]
fn view_data_has_no_data_output_or_fragment_result() {
    let fragment = build_provider_fragment().expect("core built-in fixture must assemble");
    let node = fragment
        .nodes
        .iter()
        .find(|node| node.protocol().type_id.as_str() == "yssbi.debug.view")
        .expect("View Data is registered");
    let protocol = node.protocol();
    assert_eq!(
        protocol
            .interface
            .ports
            .iter()
            .map(|port| { (port.key.as_str(), port.direction, port.kind,) })
            .collect::<Vec<_>>(),
        [
            ("enter", PortDirection::Input, PortKind::Control),
            ("data", PortDirection::Input, PortKind::Data),
            ("then", PortDirection::Output, PortKind::Control),
        ]
    );

    let implementation = node
        .implementation()
        .expect("View Data has compiler lowering")
        .as_any()
        .downcast_ref::<NodeImplementation>()
        .expect("View Data uses the native compiler lowerer");
    let data = PortAddress::declared(
        NodeId::from_uuid(uuid::Uuid::from_u128(1)),
        protocol.interface.ports[1].key.clone(),
    );
    let cancellation = CompileCancellationToken::new();
    let parameters = ValidatedNodeConfig::empty();
    let lowered = implementation
        .lowerer
        .lower(&LoweringContext {
            cancellation: &cancellation,
            node_id: data.node_id,
            protocol,
            parameters: &parameters,
            inputs: &[(data, ValueRef::new(0))],
            outputs: &[],
        })
        .expect("View Data lowering succeeds");
    let LoweredKernel::Kernel(kernel) = lowered.kernel else {
        panic!("View Data lowers to a kernel fragment");
    };
    assert!(kernel.metadata.results.is_empty());
}

#[test]
fn every_core_node_has_localized_search_terms() {
    let fragment = build_provider_fragment().expect("core built-in fixture must assemble");
    let aliases = fragment
        .messages
        .iter()
        .filter(|(_, _, message)| matches!(message, Message::Aliases(_)))
        .map(|(locale, key, _)| (*locale, *key))
        .collect::<BTreeSet<_>>();

    for node in fragment.nodes {
        let key = node
            .protocol()
            .catalog
            .aliases_key
            .as_ref()
            .expect("core nodes expose aliases and technical terms");
        assert!(aliases.contains(&("en-US", key.as_str())));
        assert!(aliases.contains(&("zh-CN", key.as_str())));
    }
}
