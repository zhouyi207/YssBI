use yss_node_catalog::LocalizedCatalogItem;
use yss_node_protocol::NodeProtocol;

#[cfg(test)]
mod tests;

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

pub(super) fn configuration_schema(
    protocol: &NodeProtocol,
    item: &LocalizedCatalogItem,
) -> serde_json::Value {
    let mut schema = serde_json::Value::from(protocol.configuration_schema());
    let fields = schema["properties"]["parameters"]["properties"]
        .as_object_mut()
        .expect("protocol configuration has parameter properties");
    for label in &item.parameters {
        if let Some(field) = fields
            .get_mut(label.key.as_ref())
            .and_then(serde_json::Value::as_object_mut)
        {
            field.insert("title".into(), serde_json::json!(label.title));
            if let Some(description) = &label.description {
                field.insert("description".into(), serde_json::json!(description));
            }
        }
    }
    schema
}
