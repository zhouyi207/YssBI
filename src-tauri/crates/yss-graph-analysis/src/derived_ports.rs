use std::collections::{BTreeMap, BTreeSet};

use yss_graph_document::{
    DynamicMemberLocator, DynamicPortBinding, FunctionParameterId, GraphDocument,
    GraphResourcePath, LastKnownPortMetadata, NodeId, OrderKey, PortAddress, PortInstanceId,
    PortRef,
};
use yss_graph_protocol::{PortInstances, TypeExpr};
use yss_graph_registry::NodeRegistry;
use yss_graph_resource_contract::ResourceCatalogSnapshot;

use crate::schema_resolution::{DerivedSchemaPortMember, derived_schema_port_members};

const FUNCTION_CALL_ARGUMENTS_RESOLVER: &str = "yssbi.project.function.call.arguments";
const FUNCTION_CALL_RESULTS_RESOLVER: &str = "yssbi.project.function.call.results";
const FUNCTION_ENTRY_PARAMETERS_RESOLVER: &str = "yssbi.project.function.entry.parameters";
const FUNCTION_RETURN_RESULTS_RESOLVER: &str = "yssbi.project.function.return.results";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DerivedPortMember {
    pub locator: DynamicMemberLocator,
    pub label: Box<str>,
    pub value_type: TypeExpr,
}

impl From<DerivedSchemaPortMember> for DerivedPortMember {
    fn from(member: DerivedSchemaPortMember) -> Self {
        Self {
            locator: member.locator,
            label: member.label,
            value_type: member.value_type,
        }
    }
}

pub(crate) fn derived_port_members(
    document: &GraphDocument,
    node_id: NodeId,
    resolver: &str,
    schemas: &BTreeMap<PortAddress, yss_graph_protocol::ResolvedSchemaFact>,
    resources: &ResourceCatalogSnapshot,
) -> Vec<DerivedPortMember> {
    let schema_members = derived_schema_port_members(node_id, resolver, schemas);
    if !schema_members.is_empty() {
        return schema_members.into_iter().map(Into::into).collect();
    }
    function_port_members(document, node_id, resolver, resources).unwrap_or_default()
}

fn function_port_members(
    document: &GraphDocument,
    node_id: NodeId,
    resolver: &str,
    resources: &ResourceCatalogSnapshot,
) -> Option<Vec<DerivedPortMember>> {
    let parameter_key = match resolver {
        FUNCTION_CALL_ARGUMENTS_RESOLVER | FUNCTION_CALL_RESULTS_RESOLVER => "target",
        FUNCTION_ENTRY_PARAMETERS_RESOLVER | FUNCTION_RETURN_RESULTS_RESOLVER => "function",
        _ => return None,
    };
    let node = document.nodes.get(&node_id)?;
    let function = node
        .parameters
        .iter()
        .find_map(|(key, value)| (key.as_str() == parameter_key).then_some(value))?
        .as_str()
        .and_then(|value| GraphResourcePath::new(value).ok())?;
    let signature = resources.function_signature(&function)?;

    if matches!(
        resolver,
        FUNCTION_CALL_RESULTS_RESOLVER | FUNCTION_RETURN_RESULTS_RESOLVER
    ) {
        let result = signature.result()?;
        return Some(vec![DerivedPortMember {
            locator: DynamicMemberLocator::FunctionParameter {
                function,
                parameter: FunctionParameterId::new("return"),
            },
            label: "Result".into(),
            value_type: yss_graph_type_mapping::type_expr_from_data_type(result).ok()?,
        }]);
    }

    signature
        .parameters()
        .iter()
        .map(|parameter| {
            Some(DerivedPortMember {
                locator: DynamicMemberLocator::FunctionParameter {
                    function: function.clone(),
                    parameter: parameter.id().clone(),
                },
                label: parameter.name().into(),
                value_type: yss_graph_type_mapping::type_expr_from_data_type(parameter.data_type())
                    .ok()?,
            })
        })
        .collect()
}

/// Materialize compile-derived interface members into a candidate draft.
///
/// The source Graph file remains untouched. Existing addresses are retained by
/// member identity, removed members with live references become orphans, and
/// newly discovered members receive deterministic addresses.
pub fn materialize_derived_port_bindings(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
) -> GraphDocument {
    let schemas = crate::schema_resolution::resolve_editor_schemas(document, registry, resources);
    let mut candidate = document.clone();
    let templates = document
        .nodes
        .values()
        .flat_map(|node| {
            registry
                .protocol(&node.node_type)
                .into_iter()
                .flat_map(move |protocol| {
                    protocol.interface.ports.iter().filter_map(move |spec| {
                        let PortInstances::Derived { resolver } = &spec.instances else {
                            return None;
                        };
                        Some((node.id, spec.key.clone(), resolver.as_str().to_owned()))
                    })
                })
        })
        .collect::<Vec<_>>();

    for (node_id, template, resolver) in templates {
        let desired = derived_port_members(document, node_id, &resolver, &schemas, resources);
        let mut existing_by_origin = candidate
            .port_bindings
            .iter()
            .filter_map(|(address, binding)| {
                if address.node_id != node_id
                    || !matches!(
                        &address.port,
                        PortRef::Instance { template: current, .. } if current == &template
                    )
                {
                    return None;
                }
                match binding {
                    DynamicPortBinding::Resolved { origin, .. }
                    | DynamicPortBinding::Orphan { origin, .. } => {
                        Some((origin.clone(), address.clone()))
                    }
                    DynamicPortBinding::UserCreated { .. } => None,
                }
            })
            .collect::<BTreeMap<_, _>>();
        let mut retained = BTreeSet::new();

        for (index, member) in desired.into_iter().enumerate() {
            let address = existing_by_origin
                .remove(&member.locator)
                .unwrap_or_else(|| {
                    derived_port_address(&candidate, node_id, &template, &member.locator)
                });
            retained.insert(address.clone());
            candidate.port_bindings.insert(
                address,
                DynamicPortBinding::Resolved {
                    origin: member.locator,
                    order: OrderKey::new(format!("{index:05}")),
                    last_known: LastKnownPortMetadata {
                        label: member.label.into(),
                        value_type: Some(member.value_type),
                    },
                },
            );
        }

        for address in existing_by_origin.into_values() {
            let Some(binding) = candidate.port_bindings.get(&address).cloned() else {
                continue;
            };
            if port_is_referenced(&candidate, &address) {
                if let DynamicPortBinding::Resolved {
                    origin,
                    order,
                    last_known,
                } = binding
                {
                    candidate.port_bindings.insert(
                        address,
                        DynamicPortBinding::Orphan {
                            origin,
                            order,
                            last_known,
                        },
                    );
                }
            } else if !retained.contains(&address) {
                candidate.port_bindings.remove(&address);
            }
        }
    }

    candidate
}

pub(crate) fn derived_port_address(
    document: &GraphDocument,
    node_id: NodeId,
    template: &yss_graph_protocol::PortKey,
    locator: &DynamicMemberLocator,
) -> PortAddress {
    for salt in 0_u32.. {
        let digest = yss_canonical_hash::hash_canonical(
            "yssbi.graph.projected-derived-port.v1",
            &(node_id, template, locator, salt),
        )
        .expect("derived port identity is canonically serializable");
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let address =
            PortAddress::instance(node_id, template.clone(), PortInstanceId::from_bytes(bytes));
        if !document.port_bindings.contains_key(&address) {
            return address;
        }
    }
    unreachable!("u32 derived port identity salt space is inexhaustible")
}

fn port_is_referenced(document: &GraphDocument, address: &PortAddress) -> bool {
    document.input_states.contains_key(address)
        || document
            .connections
            .values()
            .any(|connection| connection.output == *address || connection.input == *address)
}
