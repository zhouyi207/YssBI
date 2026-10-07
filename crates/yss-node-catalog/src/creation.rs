use yss_node_protocol::{NodeProtocol, PortCardinality, PortDirection, PortKey};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeCreationPort {
    pub key: PortKey,
    pub title: Box<str>,
    pub direction: PortDirection,
    pub count: PortCountPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PortCountPolicy {
    Fixed,
    Configurable {
        min: u16,
        max: Option<u16>,
        member_templates: Box<[PortKey]>,
    },
    Derived,
}

pub fn node_creation_ports(protocol: &NodeProtocol) -> Box<[NodeCreationPort]> {
    protocol
        .interface
        .ports
        .iter()
        .map(|port| NodeCreationPort {
            key: port.key.clone(),
            title: port.title.clone(),
            direction: port.direction,
            count: match port.cardinality {
                PortCardinality::Declared => PortCountPolicy::Fixed,
                PortCardinality::Derived { .. } => PortCountPolicy::Derived,
                PortCardinality::UserCreated { .. } => {
                    let (min, max) = protocol.interface.port_instance_bounds(port);
                    PortCountPolicy::Configurable {
                        min,
                        max,
                        member_templates: protocol
                            .interface
                            .member_group_for_template(&port.key)
                            .map_or_else(
                                || Box::new([port.key.clone()]) as Box<[_]>,
                                |group| group.templates.clone(),
                            ),
                    }
                }
            },
        })
        .collect()
}
