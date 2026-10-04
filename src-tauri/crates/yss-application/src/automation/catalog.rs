use yss_harness_contract::{
    NodeParameterCondition, NodeParameterConstraint, NodeParameterDefinition,
};
use yss_node_catalog::LocalizedCatalogItem;
use yss_node_protocol::{
    NodeProtocol, ParameterConstraint, ParameterEditorSpec, parameter_value_to_json,
};

pub(super) fn port_definitions(
    protocol: &NodeProtocol,
) -> Vec<yss_harness_contract::NodeCreationPortDefinition> {
    use yss_harness_contract::{NodeCreationPortDefinition, NodePortCountPolicy};
    yss_node_catalog::node_creation_ports(protocol)
        .into_vec()
        .into_iter()
        .map(|port| NodeCreationPortDefinition {
            key: port.key.to_string(),
            title: port.title.into_string(),
            direction: match port.direction {
                yss_node_protocol::PortDirection::Input => "input",
                yss_node_protocol::PortDirection::Output => "output",
            }
            .into(),
            count: match port.count {
                yss_node_catalog::PortCountPolicy::Fixed => NodePortCountPolicy::Fixed,
                yss_node_catalog::PortCountPolicy::Derived => NodePortCountPolicy::Derived,
                yss_node_catalog::PortCountPolicy::Configurable {
                    min,
                    max,
                    member_templates,
                } => NodePortCountPolicy::Configurable {
                    min,
                    max,
                    member_templates: member_templates.iter().map(ToString::to_string).collect(),
                },
            },
        })
        .collect()
}

pub(super) fn parameter_definitions(
    protocol: &NodeProtocol,
    item: &LocalizedCatalogItem,
) -> Vec<NodeParameterDefinition> {
    protocol
        .parameters
        .iter()
        .filter(|parameter| !matches!(parameter.editor, ParameterEditorSpec::Hidden))
        .map(|parameter| {
            let localized = item
                .parameters
                .iter()
                .find(|label| label.key.as_ref() == parameter.key.as_str());
            let json = |value| parameter_value_to_json(value, &parameter.value_type);
            NodeParameterDefinition {
                key: parameter.key.to_string(),
                title: localized.map_or_else(
                    || parameter.key.to_string(),
                    |label| label.title.to_string(),
                ),
                description: localized
                    .and_then(|label| label.description.as_ref().map(ToString::to_string)),
                value_type: serde_json::to_value(&parameter.value_type)
                    .expect("declared type expressions serialize"),
                default_value: parameter.default_json(),
                constraints: parameter
                    .constraints
                    .iter()
                    .map(|constraint| match constraint {
                        ParameterConstraint::Required => NodeParameterConstraint::Required,
                        ParameterConstraint::Positive => NodeParameterConstraint::Positive,
                        ParameterConstraint::ColumnName => NodeParameterConstraint::ColumnName,
                        ParameterConstraint::ColumnNames => NodeParameterConstraint::ColumnNames,
                        ParameterConstraint::OneOf(values) => NodeParameterConstraint::OneOf {
                            values: values.iter().map(json).collect(),
                        },
                        ParameterConstraint::IntegerRange { min, max } => {
                            NodeParameterConstraint::IntegerRange {
                                min: *min,
                                max: *max,
                            }
                        }
                        ParameterConstraint::Length { min, max } => {
                            NodeParameterConstraint::Length {
                                min: *min,
                                max: *max,
                            }
                        }
                    })
                    .collect(),
                visible_when: parameter.visible_when.as_ref().map(|condition| {
                    let selector = protocol
                        .parameters
                        .get(&condition.key)
                        .expect("registered condition selector");
                    NodeParameterCondition {
                        key: condition.key.to_string(),
                        values: condition
                            .values
                            .iter()
                            .map(|value| parameter_value_to_json(value, &selector.value_type))
                            .collect(),
                    }
                }),
                resource_bound: matches!(parameter.editor, ParameterEditorSpec::Resource { .. }),
            }
        })
        .collect()
}
