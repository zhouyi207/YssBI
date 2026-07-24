use super::{CoverageDisposition, build_provider_fragment, legacy_coverage};
use crate::node_system::catalog::localization::Message;
use std::collections::BTreeSet;

const LEGACY_CORE_NODES: &[&str] = &[
    "Value:Constants:Boolean",
    "Value:Constants:Int64",
    "Value:Constants:Float64",
    "Value:Constants:String",
    "Value:Conversion:Convert",
    "Data:Conversion:String to Categorical",
    "Data:Conversion:String to Float64",
    "Data:Conversion:String to Int64",
    "Data:Conversion:Int64 to String",
    "Data:Conversion:Float64 to String",
    "Data:Conversion:Int64 to Float64",
    "Data:Conversion:Float64 to Int64",
    "Data:Conversion:Int64 to Boolean",
    "Data:Conversion:Float64 to Boolean",
    "Data:Conversion:Categorical to String",
    "Data:Conversion:Int64 to Categorical",
    "Data:Conversion:Categorical to Int64",
    "Data:Conversion:Float64 to Categorical",
    "Data:Conversion:Categorical to Float64",
    "Math:Operators:Add (+)",
    "Math:Operators:Subtract (-)",
    "Math:Operators:Multiply (*)",
    "Math:Operators:Divide (/)",
    "Math:Functions:Ln",
    "Math:Functions:Log2",
    "Math:Functions:Log10",
    "Math:Functions:Exp",
    "Math:Functions:Sqrt",
    "Math:Functions:Square",
    "Logic:Comparison:Equal (==)",
    "Logic:Comparison:Not Equal (!=)",
    "Logic:Boolean:And (&&)",
    "Logic:Boolean:Or (||)",
    "Logic:Boolean:Not (!)",
    "Control Flow:Branch",
    "Control Flow:Sequence",
    "Control Flow:Do",
    "Control Flow:Merge",
    "Control Flow:Sleep",
    "Control Flow:For Loop",
    "Control Flow:Switch",
    "Control Flow:While Loop",
    "Debug:Print",
    "Debug:Data:View",
];

#[test]
fn legacy_core_catalog_has_an_explicit_complete_coverage_list() {
    let expected = LEGACY_CORE_NODES.iter().copied().collect::<BTreeSet<_>>();
    let actual = legacy_coverage()
        .iter()
        .map(|entry| entry.legacy_node_type)
        .collect::<BTreeSet<_>>();

    assert_eq!(actual, expected);
    assert_eq!(legacy_coverage().len(), LEGACY_CORE_NODES.len());
    assert!(
        legacy_coverage()
            .iter()
            .all(|entry| !entry.stable_ids.is_empty())
    );
}

#[test]
fn migrated_coverage_entries_are_owned_by_the_provider_fragment() {
    let fragment = build_provider_fragment();
    let node_ids = fragment
        .nodes
        .iter()
        .map(|node| node.protocol.type_id.as_str())
        .collect::<BTreeSet<_>>();

    for entry in legacy_coverage() {
        if entry.disposition == CoverageDisposition::MigratedHere {
            for stable_id in entry.stable_ids {
                assert!(
                    node_ids.contains(stable_id),
                    "migrated legacy node '{}' is missing stable node '{}'",
                    entry.legacy_node_type,
                    stable_id,
                );
            }
        }
    }
}

#[test]
fn protocols_use_unique_stable_port_and_parameter_keys() {
    let fragment = build_provider_fragment();
    for node in fragment.nodes {
        let ports = node
            .protocol
            .interface
            .ports
            .iter()
            .map(|port| port.key.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(ports.len(), node.protocol.interface.ports.len());

        let parameters = node
            .protocol
            .parameters
            .parameters
            .iter()
            .map(|parameter| parameter.key.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(parameters.len(), node.protocol.parameters.parameters.len());
    }
}

#[test]
fn every_migrated_node_has_localized_search_terms() {
    let fragment = build_provider_fragment();
    let aliases = fragment
        .messages
        .iter()
        .filter(|(_, _, message)| matches!(message, Message::Aliases(_)))
        .map(|(locale, key, _)| (*locale, *key))
        .collect::<BTreeSet<_>>();

    for node in fragment.nodes {
        let key = node
            .protocol
            .catalog
            .aliases_key
            .as_ref()
            .expect("core nodes expose aliases and technical terms");
        assert!(aliases.contains(&("en-US", key.as_str())));
        assert!(aliases.contains(&("zh-CN", key.as_str())));
    }
}
