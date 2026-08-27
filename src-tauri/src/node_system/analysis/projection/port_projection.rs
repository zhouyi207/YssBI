use super::super::ResolvedPortStatus;
use super::types::{
    EffectiveInputBindingKindDto, NodeCapabilitiesDto, PortConnectionCapabilityDto,
    PortDirectionDto, PortInstanceKindDto, PortKindDto, RelationalScalarTypeDto,
    ResolvedPortStatusDto, SchemaFieldDto, SchemaSummaryDto, SchemaSummaryKindDto, TypeSummaryDto,
};
use crate::data_contract::DataType;
use crate::graph_document::{GraphDocument, PortAddress};
use crate::node_system::document::{EffectiveInputBinding, PortAddressDto};
use crate::node_system::protocol::ResolvedSchemaFact;
use crate::node_system::protocol::{
    ConnectionsPerPort, ParameterEditorSpec, PortDirection, PortEditorSpec, PortInstances,
    PortKind, RelationalScalarType, SchemaExpr, TypeExpr,
};

pub(super) fn project_effective_input_binding(
    binding: EffectiveInputBinding,
) -> EffectiveInputBindingKindDto {
    match binding {
        EffectiveInputBinding::Connections(_) => EffectiveInputBindingKindDto::Connections,
        EffectiveInputBinding::Literal(_) => EffectiveInputBindingKindDto::Literal,
        EffectiveInputBinding::ProtocolDefault(_) => EffectiveInputBindingKindDto::ProtocolDefault,
        EffectiveInputBinding::Unbound => EffectiveInputBindingKindDto::Unbound,
    }
}
pub(super) fn project_node_capabilities(
    protocol: Option<&crate::node_system::protocol::NodeProtocol>,
) -> NodeCapabilitiesDto {
    let managed = protocol.is_some_and(|protocol| protocol.managed_role.is_some());
    NodeCapabilitiesDto {
        managed,
        can_copy: !managed,
        can_delete: !managed,
        can_edit_label: true,
        can_edit_parameters: protocol.is_some_and(|protocol| {
            protocol
                .parameters
                .parameters
                .iter()
                .any(|parameter| !matches!(parameter.editor, ParameterEditorSpec::Hidden))
        }),
        has_dynamic_ports: protocol.is_some_and(|protocol| {
            protocol
                .interface
                .ports
                .iter()
                .any(|port| !matches!(port.instances, PortInstances::Declared))
        }),
        supports_inline_literals: protocol.is_some_and(|protocol| {
            protocol
                .interface
                .ports
                .iter()
                .any(|port| matches!(port.editor, PortEditorSpec::InlineLiteral))
        }),
    }
}

pub(super) fn project_instance_kind(instances: &PortInstances) -> PortInstanceKindDto {
    match instances {
        PortInstances::Declared => PortInstanceKindDto::Declared,
        PortInstances::UserCreated { .. } => PortInstanceKindDto::UserCreated,
        PortInstances::Derived { .. } => PortInstanceKindDto::Derived,
    }
}
pub(super) fn can_remove_port(
    address: &PortAddress,
    orphan: bool,
    instances: &PortInstances,
    minimum: u16,
    instance_count: usize,
    member_complete: bool,
) -> bool {
    if !address.is_instance() {
        return false;
    }
    if orphan {
        return true;
    }
    matches!(instances, PortInstances::UserCreated { .. })
        && (!member_complete || instance_count > usize::from(minimum))
}

pub(super) fn project_connection_capability(
    document: &GraphDocument,
    address: &PortAddress,
    capability: ConnectionsPerPort,
    orphan: bool,
) -> PortConnectionCapabilityDto {
    let current = document
        .connections
        .values()
        .filter(|connection| connection.input == *address || connection.output == *address)
        .count() as u32;
    let (maximum, ordered) = match capability {
        ConnectionsPerPort::Single => (Some(1), false),
        ConnectionsPerPort::Multiple { max, ordered } => (max.map(u32::from), ordered),
    };
    PortConnectionCapabilityDto {
        current,
        maximum,
        ordered,
        can_append: !orphan && maximum.is_none_or(|maximum| current < maximum),
        can_replace: !orphan && capability == ConnectionsPerPort::Single && current == 1,
        can_move: !orphan && current > 0,
    }
}
pub(super) fn project_type_summary(value: &TypeExpr) -> TypeSummaryDto {
    TypeSummaryDto {
        display: type_display(value).into(),
        resolved: type_is_resolved(value),
        data_type: project_data_type(value),
        internal_type_expr: Some(value.clone()),
    }
}

pub(crate) fn project_data_type(value: &TypeExpr) -> Option<DataType> {
    match value {
        TypeExpr::Concrete(id) => Some(match id.as_str() {
            "core.bool" => DataType::Boolean,
            "core.int64" => DataType::Int64,
            "core.float64" => DataType::Float64,
            "core.string" => DataType::String,
            "core.date" => DataType::Date,
            "core.datetime" => DataType::Datetime,
            "core.time" => DataType::Time,
            "core.categorical" => DataType::Categorical,
            "core.object" => DataType::Object,
            "tabular.dataframe" => DataType::DataFrame,
            semantic_id => DataType::Struct(semantic_id.to_owned()),
        }),
        TypeExpr::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.data_series" && arguments.len() == 1 => {
            project_data_type(&arguments[0]).map(|element| DataType::DataSeries(Box::new(element)))
        }
        TypeExpr::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.array" && arguments.len() == 1 => {
            project_data_type(&arguments[0]).map(|element| DataType::Array(Box::new(element)))
        }
        TypeExpr::Applied { .. } => None,
        TypeExpr::Union(values) if !values.is_empty() => values
            .iter()
            .map(project_data_type)
            .collect::<Option<Vec<_>>>()
            .map(DataType::one_of),
        TypeExpr::Unknown => None,
        TypeExpr::Union(_) | TypeExpr::Generic(_) => None,
    }
}

fn type_display(value: &TypeExpr) -> String {
    match value {
        TypeExpr::Concrete(id) => id.as_str().to_owned(),
        TypeExpr::Generic(id) => id.as_str().to_owned(),
        TypeExpr::Applied {
            constructor,
            arguments,
        } => format!(
            "{}<{}>",
            constructor.as_str(),
            arguments
                .iter()
                .map(type_display)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeExpr::Union(values) => values
            .iter()
            .map(type_display)
            .collect::<Vec<_>>()
            .join(" | "),
        TypeExpr::Unknown => "unknown".to_owned(),
    }
}

fn type_is_resolved(value: &TypeExpr) -> bool {
    match value {
        TypeExpr::Concrete(_) => true,
        TypeExpr::Applied { arguments, .. } | TypeExpr::Union(arguments) => {
            arguments.iter().all(type_is_resolved)
        }
        TypeExpr::Generic(_) | TypeExpr::Unknown => false,
    }
}

pub(super) fn project_schema_summary(
    value: &SchemaExpr,
    resolved: Option<&ResolvedSchemaFact>,
) -> SchemaSummaryDto {
    let kind = match value {
        SchemaExpr::Input(_) => SchemaSummaryKindDto::Input,
        SchemaExpr::Project { .. } => SchemaSummaryKindDto::Project,
        SchemaExpr::Append { .. } => SchemaSummaryKindDto::Append,
        SchemaExpr::Rename { .. } => SchemaSummaryKindDto::Rename,
        SchemaExpr::Filter { .. } => SchemaSummaryKindDto::Filter,
        SchemaExpr::Derived { .. } => SchemaSummaryKindDto::Derived,
    };
    let fields = resolved
        .into_iter()
        .flat_map(|fact| fact.fields.iter())
        .map(|field| SchemaFieldDto {
            name: field.name.0.clone(),
            scalar_type: relational_scalar_type_dto(field.scalar_type),
        })
        .collect();
    SchemaSummaryDto { kind, fields }
}
pub(super) fn relational_scalar_type_dto(value: RelationalScalarType) -> RelationalScalarTypeDto {
    match value {
        RelationalScalarType::Boolean => RelationalScalarTypeDto::Boolean,
        RelationalScalarType::Int64 => RelationalScalarTypeDto::Int64,
        RelationalScalarType::Float64 => RelationalScalarTypeDto::Float64,
        RelationalScalarType::String => RelationalScalarTypeDto::String,
        RelationalScalarType::Date => RelationalScalarTypeDto::Date,
        RelationalScalarType::DateTime => RelationalScalarTypeDto::DateTime,
        RelationalScalarType::Unknown => RelationalScalarTypeDto::Unknown,
    }
}
pub(super) fn project_address(address: &PortAddress) -> PortAddressDto {
    address.into()
}
impl From<PortDirection> for PortDirectionDto {
    fn from(value: PortDirection) -> Self {
        match value {
            PortDirection::Input => Self::Input,
            PortDirection::Output => Self::Output,
        }
    }
}

impl From<PortKind> for PortKindDto {
    fn from(value: PortKind) -> Self {
        match value {
            PortKind::Data => Self::Data,
            PortKind::Control => Self::Control,
            PortKind::Effect => Self::Effect,
        }
    }
}

impl From<ResolvedPortStatus> for ResolvedPortStatusDto {
    fn from(value: ResolvedPortStatus) -> Self {
        match value {
            ResolvedPortStatus::Resolved => Self::Resolved,
            ResolvedPortStatus::Orphan => Self::Orphan,
        }
    }
}
