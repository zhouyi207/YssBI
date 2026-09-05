use yss_graph_document::{
    DynamicMemberLocator, FunctionParameterId, GraphDocument, GraphResourcePath, NodeId,
    PortAddress, PortInstanceId,
};
use yss_graph_protocol::TypeExpr;
use yss_graph_resource_contract::ResourceCatalogSnapshot;

use crate::schema_resolution::{DerivedSchemaPortMember, derived_schema_port_members};

const FUNCTION_CALL_ARGUMENTS_RESOLVER: &str = "yssbi.project.function.call.arguments";
const FUNCTION_CALL_RESULTS_RESOLVER: &str = "yssbi.project.function.call.results";
const FUNCTION_ENTRY_PARAMETERS_RESOLVER: &str = "yssbi.project.function.entry.parameters";
const FUNCTION_RETURN_RESULTS_RESOLVER: &str = "yssbi.project.function.return.results";

pub(crate) fn supports_resolver(resolver: &str) -> bool {
    matches!(
        resolver,
        FUNCTION_CALL_ARGUMENTS_RESOLVER
            | FUNCTION_CALL_RESULTS_RESOLVER
            | FUNCTION_ENTRY_PARAMETERS_RESOLVER
            | FUNCTION_RETURN_RESULTS_RESOLVER
            | "yssbi.dataframe.interface.columns"
    )
}

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
    schemas: &crate::schema_resolution::SchemaResolution,
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

pub(crate) fn port_is_referenced(document: &GraphDocument, address: &PortAddress) -> bool {
    document
        .input_states
        .get(address)
        .is_some_and(|state| state.literal_override.is_some())
        || document
            .connections
            .values()
            .any(|connection| connection.output == *address || connection.input == *address)
}
