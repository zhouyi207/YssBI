//! DataFrame node protocols staged for aggregation into the built-in provider.
//!
//! This module intentionally depends only on the new node-system IR. It does not
//! use legacy graph authoring, pin-reconciliation, or execution types.

mod families;

use super::builtin::{
    BuiltinAssemblyError, ProviderFragment, assembled_interface, assembled_parameters, iid, leaf,
    sid,
};
use super::localization::{Aliases, Message, Text};
use crate::node_system::compiler::{
    FragmentMetadata, FragmentResult, LoweredKernel, LoweredNode, LoweringContext, LoweringError,
    LoweringInvariant, NodeImplementation, NodeLowerer, RelationalInputBinding,
    RelationalNodeFragment,
};
use crate::node_system::document::PortRef;
use crate::node_system::plan::{
    CompiledParameterHandle, RelationalBackendId, RelationalExpression, RelationalFragmentId,
    RelationalLiteral, RelationalOperator, RelationalOperatorIndex, RelationalProjection,
    RelationalRename,
};
use crate::node_system::protocol::dataframe::{
    FilterLiteral, FilterOperator, FilterPredicate, ProjectColumns,
};
use crate::node_system::protocol::*;
use crate::node_system::registry::{
    CategoryRegistration, NominalValueHandle, ProviderRegistration, RegisteredNode,
    TypeRegistration,
};
use std::sync::Arc;

#[cfg(test)]
pub use families::LEGACY_NODE_IDS;
use families::{InterfaceKind, NODES, NodeSpec};

pub use crate::node_system::compiler::DATAFRAME_COLUMNS_RESOLVER;
pub const DATAFRAME_RESOURCE_SCHEMA_RESOLVER: &str = "yssbi.dataframe.schema.resource";
pub const DATAFRAME_PANEL_SCHEMA_RESOLVER: &str = "yssbi.dataframe.schema.panel";

pub(crate) fn build_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError> {
    let mut messages = Vec::new();
    add_shared_messages(&mut messages);
    let nodes = NODES
        .iter()
        .map(|spec| {
            add_node_messages(&mut messages, spec);
            registered_node(spec)
        })
        .collect::<Result<Vec<_>, BuiltinAssemblyError>>()?;

    Ok(ProviderFragment {
        types: dataframe_types()?,
        categories: dataframe_categories()?,
        interface_resolvers: vec![sid(DATAFRAME_COLUMNS_RESOLVER, InterfaceResolverId::new)?],
        schema_resolvers: vec![
            sid(DATAFRAME_RESOURCE_SCHEMA_RESOLVER, SchemaResolverId::new)?,
            sid(DATAFRAME_PANEL_SCHEMA_RESOLVER, SchemaResolverId::new)?,
        ],
        nodes,
        messages,
        ..ProviderFragment::default()
    })
}

fn registered_node(spec: &NodeSpec) -> Result<RegisteredNode, BuiltinAssemblyError> {
    let protocol = protocol(spec)?;
    Ok(match spec.interface {
        InterfaceKind::DataframeSource => RegisteredNode::leaf(
            Arc::new(protocol),
            Arc::new(NodeImplementation::new(SourceLowerer)),
        ),
        InterfaceKind::Limit => RegisteredNode::leaf(
            Arc::new(protocol),
            Arc::new(NodeImplementation::new(LimitLowerer)),
        ),
        InterfaceKind::Rename => RegisteredNode::leaf(
            Arc::new(protocol),
            Arc::new(NodeImplementation::new(RenameLowerer)),
        ),
        InterfaceKind::Project => RegisteredNode::leaf(
            Arc::new(protocol),
            Arc::new(NodeImplementation::new(ProjectLowerer(None))),
        ),
        InterfaceKind::FilterRows => RegisteredNode::leaf(
            Arc::new(protocol),
            Arc::new(NodeImplementation::new(FilterRowsLowerer(None))),
        ),
        _ => leaf(protocol, spec.kernel),
    })
}

fn protocol(spec: &NodeSpec) -> Result<NodeProtocol, BuiltinAssemblyError> {
    let (ports, parameters) = interface(spec.interface)?;
    Ok(NodeProtocol {
        type_id: sid(spec.id, NodeTypeId::new)?,
        catalog: NodeCatalogProtocol {
            title_key: node_key(spec.id, "title")?,
            description_key: Some(node_key(spec.id, "description")?),
            documentation_key: Some(node_key(spec.id, "documentation")?),
            aliases_key: Some(node_key(spec.id, "aliases")?),
            category_id: sid(category(spec.interface), NodeCategoryId::new)?,
            icon_id: sid("builtin.dataframe", IconId::new)?,
            style_id: sid("builtin.dataframe", NodeStyleId::new)?,
            hidden: false,
        },
        interface: assembled_interface(spec.id, ports, vec![], vec![], vec![])?,
        parameters: assembled_parameters(spec.id, parameters)?,
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            purity: Purity::Pure,
            evaluation: EvaluationPolicy::DemandDriven,
            cache: CachePolicy::PerRun,
            effects: EffectSemantics::None,
            idempotent: false,
            retry: None,
        },
        scope: NodeScope::Any,
        managed_role: None,
    })
}

fn interface(
    kind: InterfaceKind,
) -> Result<(Vec<PortSpec>, Vec<ParameterSpec>), BuiltinAssemblyError> {
    use InterfaceKind::*;
    match kind {
        DataframeSource => Ok((
            vec![streaming_output(
                "dataframe",
                dataframe_type()?,
                Some(derived_schema(DATAFRAME_RESOURCE_SCHEMA_RESOLVER, vec![])?),
            )?],
            vec![resource_parameter("dataframe")?],
        )),
        Limit => Ok((
            vec![
                streaming_input("source", dataframe_type()?, None)?,
                streaming_output(
                    "result",
                    dataframe_type()?,
                    Some(SchemaExpr::Input(port_key("source")?)),
                )?,
            ],
            vec![bounded_positive_integer_parameter("rows", 100, 1_000_000)?],
        )),
        Rename => Ok((
            vec![
                streaming_input("source", dataframe_type()?, None)?,
                streaming_output(
                    "result",
                    dataframe_type()?,
                    Some(SchemaExpr::Rename {
                        input: Box::new(SchemaExpr::Input(port_key("source")?)),
                        mapping: RenameExpr::FromParameters {
                            from: sid("from", ParameterKey::new)?,
                            to: sid("to", ParameterKey::new)?,
                        },
                    }),
                )?,
            ],
            vec![
                required_text_parameter("from")?,
                required_text_parameter("to")?,
            ],
        )),
        Project => Ok((
            relational_ports(SchemaExpr::Project {
                input: Box::new(SchemaExpr::Input(port_key("source")?)),
                columns: ColumnSelectionExpr::FromParameter(sid("columns", ParameterKey::new)?),
            })?,
            vec![nominal_parameter(
                "columns",
                crate::node_system::protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
            )?],
        )),
        FilterRows => Ok((
            relational_ports(SchemaExpr::Filter {
                input: Box::new(SchemaExpr::Input(port_key("source")?)),
                predicate: Some(sid("predicate", ParameterKey::new)?),
            })?,
            vec![nominal_parameter(
                "predicate",
                crate::node_system::protocol::dataframe::FILTER_PREDICATE_TYPE_ID,
            )?],
        )),
        Decompose => Ok((
            vec![
                data_input("dataframe", dataframe_type()?, None)?,
                derived_output("columns", series_type()?, DATAFRAME_COLUMNS_RESOLVER)?,
            ],
            vec![],
        )),
        Combine => Ok((
            vec![
                user_input("series", series_type()?, 1)?,
                data_output(
                    "dataframe",
                    dataframe_type()?,
                    Some(SchemaExpr::Append {
                        inputs: vec![SchemaExpr::Input(port_key("series")?)],
                    }),
                )?,
            ],
            vec![],
        )),
        Filter => Ok((
            vec![
                data_input("source", dataframe_type()?, None)?,
                data_input("condition", bool_series_type()?, None)?,
                data_output(
                    "result",
                    dataframe_type()?,
                    Some(SchemaExpr::Filter {
                        input: Box::new(SchemaExpr::Input(port_key("source")?)),
                        predicate: None,
                    }),
                )?,
            ],
            vec![],
        )),
        SeriesSelect => Ok((
            vec![
                data_input("dataframe", dataframe_type()?, None)?,
                data_output("series", series_type()?, None)?,
            ],
            vec![column_parameter("column")?],
        )),
        IntRange => Ok((
            vec![
                scalar_input("start", "core.int64")?,
                scalar_input("end", "core.int64")?,
                scalar_input("step", "core.int64")?,
                data_output("series", int_series_type()?, None)?,
            ],
            vec![],
        )),
        SeriesUnaryScalar => Ok((
            vec![
                data_input("series", series_type()?, None)?,
                data_output("value", float_type()?, None)?,
            ],
            vec![],
        )),
        SeriesCompare => Ok((
            vec![
                data_input("left", series_type()?, None)?,
                data_input("right", series_or_scalar_type()?, None)?,
                data_output("result", bool_series_type()?, None)?,
            ],
            vec![],
        )),
        Standardize => Ok((
            vec![
                data_input("series", float_series_type()?, None)?,
                data_output("standardized", float_series_type()?, None)?,
                data_output("mean", float_type()?, None)?,
                data_output("standard_deviation", float_type()?, None)?,
            ],
            vec![],
        )),
        InverseStandardize => Ok((
            vec![
                data_input("standardized", float_series_type()?, None)?,
                scalar_input("mean", "core.float64")?,
                scalar_input("standard_deviation", "core.float64")?,
                data_output("series", float_series_type()?, None)?,
            ],
            vec![],
        )),
        DummyInfo => Ok((
            vec![
                data_input("source", series_type()?, None)?,
                data_output("result", series_type()?, None)?,
            ],
            vec![text_parameter("base_level", false)?],
        )),
        TimeAlign => Ok((
            vec![
                data_input("dataframe", dataframe_type()?, None)?,
                data_input("time", series_type()?, None)?,
                data_output(
                    "aligned",
                    dataframe_type()?,
                    Some(SchemaExpr::Input(port_key("dataframe")?)),
                )?,
            ],
            vec![select_parameter("frequency")?],
        )),
        TimeUnary => Ok((
            vec![
                data_input("series", float_series_type()?, None)?,
                data_output("result", float_series_type()?, None)?,
            ],
            vec![positive_integer_parameter("order", 1)?],
        )),
        TimeWindow => Ok((
            vec![
                data_input("series", float_series_type()?, None)?,
                data_output("result", float_series_type()?, None)?,
            ],
            vec![positive_integer_parameter("window", 1)?],
        )),
        PanelAlign => Ok((
            vec![
                data_input("dataframe", dataframe_type()?, None)?,
                data_input("entity", series_type()?, None)?,
                data_input("time", series_type()?, None)?,
                data_output(
                    "aligned",
                    dataframe_type()?,
                    Some(derived_schema(
                        DATAFRAME_PANEL_SCHEMA_RESOLVER,
                        vec![SchemaDependency::Port(port_key("dataframe")?)],
                    )?),
                )?,
            ],
            vec![],
        )),
        PanelDifference => Ok((
            vec![
                data_input("aligned", dataframe_type()?, None)?,
                data_input("series", float_series_type()?, None)?,
                data_output("result", float_series_type()?, None)?,
            ],
            vec![positive_integer_parameter("order", 1)?],
        )),
    }
}

fn relational_ports(result_schema: SchemaExpr) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    Ok(vec![
        streaming_input("source", dataframe_type()?, None)?,
        streaming_output("result", dataframe_type()?, Some(result_schema))?,
    ])
}

fn data_input(
    key: &'static str,
    value_type: TypeExpr,
    schema: Option<SchemaExpr>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(
        key,
        PortDirection::Input,
        value_type,
        PortInstances::Declared,
        schema,
    )
}

fn streaming_input(
    key: &'static str,
    value_type: TypeExpr,
    schema: Option<SchemaExpr>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    let mut spec = data_input(key, value_type, schema)?;
    spec.consumption = Some(InputConsumption::Streaming);
    Ok(spec)
}

fn scalar_input(
    key: &'static str,
    type_id: &'static str,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_input(key, concrete(type_id)?, None)
}

fn user_input(
    key: &'static str,
    value_type: TypeExpr,
    min: u16,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(
        key,
        PortDirection::Input,
        value_type,
        PortInstances::UserCreated { min, max: None },
        None,
    )
}

fn data_output(
    key: &'static str,
    value_type: TypeExpr,
    schema: Option<SchemaExpr>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(
        key,
        PortDirection::Output,
        value_type,
        PortInstances::Declared,
        schema,
    )
}

fn streaming_output(
    key: &'static str,
    value_type: TypeExpr,
    schema: Option<SchemaExpr>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    let mut spec = data_output(key, value_type, schema)?;
    spec.production = Some(OutputProduction::Streaming);
    Ok(spec)
}

fn derived_output(
    key: &'static str,
    value_type: TypeExpr,
    resolver: &'static str,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(
        key,
        PortDirection::Output,
        value_type,
        PortInstances::Derived {
            resolver: sid(resolver, InterfaceResolverId::new)?,
        },
        None,
    )
}

fn port(
    key: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
    instances: PortInstances,
    schema: Option<SchemaExpr>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: port_key(key)?,
        label_key: iid(leak(format!("ports.{key}.label")))?,
        direction,
        kind: PortKind::Data,
        value_type,
        instances,
        connections: ConnectionsPerPort::Single,
        input_binding: (direction == PortDirection::Input).then_some(InputBindingSpec {
            literal_policy: LiteralPolicy::Forbidden,
            default_value: None,
        }),
        consumption: (direction == PortDirection::Input)
            .then_some(InputConsumption::FullyMaterialized),
        production: (direction == PortDirection::Output)
            .then_some(OutputProduction::FullyMaterialized),
        editor: PortEditorSpec::Default,
        schema,
    })
}

fn resource_parameter(key: &'static str) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.string")?,
        ParameterEditorSpec::Resource,
        None,
        vec![ParameterConstraint::Required],
    )
}

fn column_parameter(key: &'static str) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.string")?,
        ParameterEditorSpec::Select,
        None,
        vec![ParameterConstraint::Required],
    )
}

fn nominal_parameter(
    key: &'static str,
    type_id: &'static str,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete(type_id)?,
        ParameterEditorSpec::Auto,
        None,
        vec![ParameterConstraint::Required],
    )
}

fn text_parameter(
    key: &'static str,
    multiline: bool,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.string")?,
        ParameterEditorSpec::Text { multiline },
        None,
        vec![],
    )
}

fn required_text_parameter(key: &'static str) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.string")?,
        ParameterEditorSpec::Text { multiline: false },
        None,
        vec![ParameterConstraint::Required],
    )
}

fn select_parameter(key: &'static str) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.string")?,
        ParameterEditorSpec::Select,
        None,
        vec![ParameterConstraint::Required],
    )
}

fn positive_integer_parameter(
    key: &'static str,
    default: i64,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.int64")?,
        ParameterEditorSpec::Number,
        Some(ParameterValue {
            value_type: concrete("core.int64")?,
            value: Value::Integer(default),
        }),
        vec![ParameterConstraint::IntegerRange {
            min: Some(1),
            max: None,
        }],
    )
}

fn bounded_positive_integer_parameter(
    key: &'static str,
    default: i64,
    max: i64,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    let mut spec = positive_integer_parameter(key, default)?;
    spec.constraints = vec![ParameterConstraint::IntegerRange {
        min: Some(1),
        max: Some(max),
    }];
    Ok(spec)
}

fn parameter(
    key: &'static str,
    value_type: TypeExpr,
    editor: ParameterEditorSpec,
    default_value: Option<ParameterValue>,
    constraints: Vec<ParameterConstraint>,
) -> Result<ParameterSpec, BuiltinAssemblyError> {
    Ok(ParameterSpec {
        key: sid(key, ParameterKey::new)?,
        title_key: iid(leak(format!("parameters.{key}.title")))?,
        description_key: Some(iid(leak(format!("parameters.{key}.description")))?),
        value_type,
        default_value,
        constraints,
        editor,
    })
}

fn dataframe_types() -> Result<Vec<TypeRegistration>, BuiltinAssemblyError> {
    [
        ("tabular.dataframe", "types.dataframe.title"),
        ("tabular.series", "types.series.title"),
        (
            crate::node_system::protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
            "types.dataframe_project_columns.title",
        ),
        (
            crate::node_system::protocol::dataframe::FILTER_PREDICATE_TYPE_ID,
            "types.dataframe_filter_predicate.title",
        ),
    ]
    .into_iter()
    .map(|(id, title)| {
        Ok(TypeRegistration {
            id: sid(id, TypeId::new)?,
            title_key: iid(title)?,
            classes: Default::default(),
        })
    })
    .collect()
}

fn dataframe_categories() -> Result<Vec<CategoryRegistration>, BuiltinAssemblyError> {
    [
        ("dataframe", None, 60),
        ("dataframe.series", Some("dataframe"), 61),
        ("dataframe.timeseries", Some("dataframe"), 62),
        ("dataframe.panel", Some("dataframe"), 63),
    ]
    .into_iter()
    .map(|(id, parent, order)| {
        Ok(CategoryRegistration {
            id: sid(id, NodeCategoryId::new)?,
            title_key: iid(leak(format!("categories.{id}.title")))?,
            parent: parent
                .map(|value| sid(value, NodeCategoryId::new))
                .transpose()?,
            order,
        })
    })
    .collect()
}

fn category(kind: InterfaceKind) -> &'static str {
    match kind {
        InterfaceKind::DataframeSource
        | InterfaceKind::Limit
        | InterfaceKind::Rename
        | InterfaceKind::Project
        | InterfaceKind::FilterRows
        | InterfaceKind::Decompose
        | InterfaceKind::Combine
        | InterfaceKind::Filter => "dataframe",
        InterfaceKind::TimeAlign | InterfaceKind::TimeUnary | InterfaceKind::TimeWindow => {
            "dataframe.timeseries"
        }
        InterfaceKind::PanelAlign | InterfaceKind::PanelDifference => "dataframe.panel",
        _ => "dataframe.series",
    }
}

fn lowering_parameter_key(value: &'static str) -> Result<ParameterKey, LoweringError> {
    ParameterKey::new(value)
        .map_err(|_| LoweringError::internal(LoweringInvariant::InvalidStaticHandle))
}

struct SourceLowerer;

impl NodeLowerer for SourceLowerer {
    fn lower(&self, context: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        let key = lowering_parameter_key("dataframe")?;
        let resource = context.parameters.resource(&key).cloned().ok_or_else(|| {
            LoweringError::internal(LoweringInvariant::InvalidPreparedConfiguration)
        })?;
        let relation = resource.as_str().into();
        relational_node(
            context,
            vec![RelationalOperator::Source { resource, relation }],
            RelationalOperatorIndex::new(0),
            Box::new([]),
            FragmentMetadata::default(),
        )
    }
}

pub(crate) struct DataframeNominalHandles {
    pub project_columns: NominalValueHandle<ProjectColumns>,
    pub filter_predicate: NominalValueHandle<FilterPredicate>,
}

pub(crate) fn bind_nominal_handles(
    provider: &mut ProviderRegistration,
    handles: DataframeNominalHandles,
) {
    for node in &mut provider.nodes {
        let lowerer: Option<NodeImplementation> = match node.protocol().type_id.as_str() {
            "yssbi.dataframe.project" => Some(NodeImplementation::new(ProjectLowerer(Some(
                handles.project_columns.clone(),
            )))),
            "yssbi.dataframe.filter.rows" => Some(NodeImplementation::new(FilterRowsLowerer(
                Some(handles.filter_predicate.clone()),
            ))),
            _ => None,
        };
        if let Some(lowerer) = lowerer {
            *node = RegisteredNode::leaf(Arc::new(node.protocol().clone()), Arc::new(lowerer));
        }
    }
}

struct ProjectLowerer(Option<NominalValueHandle<ProjectColumns>>);

impl NodeLowerer for ProjectLowerer {
    fn lower(&self, context: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        let handle = self.0.as_ref().ok_or_else(|| {
            LoweringError::internal(LoweringInvariant::InvalidPreparedConfiguration)
        })?;
        let columns = prepared_nominal_parameter(context, "columns", handle)?;
        let columns = columns
            .as_slice()
            .iter()
            .map(|name| RelationalProjection {
                name: name.clone(),
                expression: RelationalExpression::Column(name.clone()),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        relational_transform_node(
            context,
            RelationalOperator::Project {
                input: RelationalOperatorIndex::new(0),
                columns,
            },
        )
    }
}

struct FilterRowsLowerer(Option<NominalValueHandle<FilterPredicate>>);

impl NodeLowerer for FilterRowsLowerer {
    fn lower(&self, context: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        let handle = self.0.as_ref().ok_or_else(|| {
            LoweringError::internal(LoweringInvariant::InvalidPreparedConfiguration)
        })?;
        let predicate = prepared_nominal_parameter(context, "predicate", handle)?;
        relational_transform_node(
            context,
            RelationalOperator::Filter {
                input: RelationalOperatorIndex::new(0),
                predicate: lower_filter_predicate(predicate)?,
            },
        )
    }
}

fn prepared_nominal_parameter<T: Clone + Send + Sync + 'static>(
    context: &LoweringContext<'_>,
    key: &'static str,
    handle: &NominalValueHandle<T>,
) -> Result<T, LoweringError> {
    let parameter_key = lowering_parameter_key(key)?;
    context
        .parameters
        .nominal(&parameter_key, handle)
        .cloned()
        .ok_or_else(|| LoweringError::internal(LoweringInvariant::InvalidPreparedConfiguration))
}

fn lower_filter_predicate(
    predicate: FilterPredicate,
) -> Result<RelationalExpression, LoweringError> {
    let column = RelationalExpression::Column(predicate.column);
    match predicate.operator {
        FilterOperator::IsNull => Ok(RelationalExpression::IsNull(Box::new(column))),
        FilterOperator::IsNotNull => Ok(RelationalExpression::Not(Box::new(
            RelationalExpression::IsNull(Box::new(column)),
        ))),
        operator => {
            let value = predicate.value.ok_or_else(|| {
                LoweringError::internal(LoweringInvariant::InvalidPreparedConfiguration)
            })?;
            let literal = RelationalExpression::Literal(lower_filter_literal(value));
            let column = Box::new(column);
            let literal = Box::new(literal);
            Ok(match operator {
                FilterOperator::Equal => RelationalExpression::Equal(column, literal),
                FilterOperator::NotEqual => RelationalExpression::NotEqual(column, literal),
                FilterOperator::LessThan => RelationalExpression::LessThan(column, literal),
                FilterOperator::LessThanOrEqual => {
                    RelationalExpression::LessThanOrEqual(column, literal)
                }
                FilterOperator::GreaterThan => RelationalExpression::GreaterThan(column, literal),
                FilterOperator::GreaterThanOrEqual => {
                    RelationalExpression::GreaterThanOrEqual(column, literal)
                }
                FilterOperator::IsNull => RelationalExpression::IsNull(column),
                FilterOperator::IsNotNull => {
                    RelationalExpression::Not(Box::new(RelationalExpression::IsNull(column)))
                }
            })
        }
    }
}

fn lower_filter_literal(literal: FilterLiteral) -> RelationalLiteral {
    match literal {
        FilterLiteral::Boolean(value) => RelationalLiteral::Boolean(value),
        FilterLiteral::Integer(value) => RelationalLiteral::Integer(value),
        FilterLiteral::Decimal(value) => RelationalLiteral::Decimal(value),
        FilterLiteral::String(value) => RelationalLiteral::String(value),
    }
}

fn relational_transform_node(
    context: &LoweringContext<'_>,
    operator: RelationalOperator,
) -> Result<LoweredNode, LoweringError> {
    let input = context
        .inputs
        .iter()
        .find(|(address, _)| {
            matches!(&address.port, PortRef::Declared { key } if key.as_str() == "source")
        })
        .map(|(address, _)| address.clone())
        .ok_or_else(|| LoweringError::internal(LoweringInvariant::MissingMaterializedPort))?;
    let result = context
        .outputs
        .iter()
        .find(|(address, _)| {
            matches!(&address.port, PortRef::Declared { key } if key.as_str() == "result")
        })
        .map(|(address, _)| address.clone())
        .ok_or_else(|| LoweringError::internal(LoweringInvariant::MissingMaterializedPort))?;
    relational_node(
        context,
        vec![
            RelationalOperator::Input {
                name: "source".into(),
            },
            operator,
        ],
        RelationalOperatorIndex::new(1),
        Box::new([RelationalInputBinding {
            port: input,
            operator: RelationalOperatorIndex::new(0),
        }]),
        FragmentMetadata {
            results: Box::new([FragmentResult {
                name: format!("node.{}.result", context.node_id).into(),
                output: result,
            }]),
            ..FragmentMetadata::default()
        },
    )
}

struct RenameLowerer;

impl NodeLowerer for RenameLowerer {
    fn lower(&self, context: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        let from = rename_parameter(context, "from")?;
        let to = rename_parameter(context, "to")?;
        let input = context
            .inputs
            .iter()
            .find(|(address, _)| {
                matches!(&address.port, PortRef::Declared { key } if key.as_str() == "source")
            })
            .map(|(address, _)| address.clone())
            .ok_or_else(|| LoweringError::internal(LoweringInvariant::MissingMaterializedPort))?;
        let result = context
            .outputs
            .iter()
            .find(|(address, _)| {
                matches!(&address.port, PortRef::Declared { key } if key.as_str() == "result")
            })
            .map(|(address, _)| address.clone())
            .ok_or_else(|| LoweringError::internal(LoweringInvariant::MissingMaterializedPort))?;
        relational_node(
            context,
            vec![
                RelationalOperator::Input {
                    name: "source".into(),
                },
                RelationalOperator::Rename {
                    input: RelationalOperatorIndex::new(0),
                    columns: Box::new([RelationalRename { from, to }]),
                },
            ],
            RelationalOperatorIndex::new(1),
            Box::new([RelationalInputBinding {
                port: input,
                operator: RelationalOperatorIndex::new(0),
            }]),
            FragmentMetadata {
                results: Box::new([FragmentResult {
                    name: format!("node.{}.result", context.node_id).into(),
                    output: result,
                }]),
                ..FragmentMetadata::default()
            },
        )
    }
}

fn rename_parameter(
    context: &LoweringContext<'_>,
    key: &'static str,
) -> Result<Box<str>, LoweringError> {
    let parameter_key = lowering_parameter_key(key)?;
    context
        .parameters
        .string(&parameter_key)
        .map(Into::into)
        .ok_or_else(|| LoweringError::internal(LoweringInvariant::InvalidPreparedConfiguration))
}

struct LimitLowerer;

impl NodeLowerer for LimitLowerer {
    fn lower(&self, context: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        let key = lowering_parameter_key("rows")?;
        let rows = context.parameters.int64(&key).ok_or_else(|| {
            LoweringError::internal(LoweringInvariant::InvalidPreparedConfiguration)
        })?;
        let rows = u64::try_from(rows).map_err(|_| {
            LoweringError::internal(LoweringInvariant::InvalidPreparedConfiguration)
        })?;
        let input = context
            .inputs
            .iter()
            .find(|(address, _)| {
                matches!(&address.port, PortRef::Declared { key } if key.as_str() == "source")
            })
            .map(|(address, _)| address.clone())
            .ok_or_else(|| LoweringError::internal(LoweringInvariant::MissingMaterializedPort))?;
        let result = context
            .outputs
            .iter()
            .find(|(address, _)| {
                matches!(&address.port, PortRef::Declared { key } if key.as_str() == "result")
            })
            .map(|(address, _)| address.clone())
            .ok_or_else(|| LoweringError::internal(LoweringInvariant::MissingMaterializedPort))?;
        relational_node(
            context,
            vec![
                RelationalOperator::Input {
                    name: "source".into(),
                },
                RelationalOperator::Limit {
                    input: RelationalOperatorIndex::new(0),
                    rows,
                },
            ],
            RelationalOperatorIndex::new(1),
            Box::new([RelationalInputBinding {
                port: input,
                operator: RelationalOperatorIndex::new(0),
            }]),
            FragmentMetadata {
                results: Box::new([FragmentResult {
                    name: format!("node.{}.result", context.node_id).into(),
                    output: result,
                }]),
                ..FragmentMetadata::default()
            },
        )
    }
}

fn relational_node(
    context: &LoweringContext<'_>,
    operators: Vec<RelationalOperator>,
    root: RelationalOperatorIndex,
    inputs: Box<[RelationalInputBinding]>,
    metadata: FragmentMetadata,
) -> Result<LoweredNode, LoweringError> {
    Ok(LoweredNode {
        kernel: LoweredKernel::Relational(RelationalNodeFragment {
            backend: RelationalBackendId::new("relational.default")
                .map_err(|_| LoweringError::internal(LoweringInvariant::InvalidStaticHandle))?,
            fragment: crate::node_system::compiler::relational::RelationalFragment {
                id: RelationalFragmentId::new(format!("node.{}", context.node_id))
                    .map_err(|_| LoweringError::internal(LoweringInvariant::InvalidStaticHandle))?,
                operators: operators.into_boxed_slice(),
                root,
            },
            inputs,
            metadata,
        }),
        parameters: CompiledParameterHandle::new(format!("node.{}", context.node_id))
            .map_err(|_| LoweringError::internal(LoweringInvariant::InvalidStaticHandle))?,
    })
}

fn dataframe_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete("tabular.dataframe")
}
fn series_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete("tabular.series")
}
fn float_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete("core.float64")
}
fn int_series_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    series_type()
}
fn float_series_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    series_type()
}
fn bool_series_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    series_type()
}
fn series_or_scalar_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(TypeExpr::Union(vec![series_type()?, float_type()?]))
}
fn concrete(id: &'static str) -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(TypeExpr::Concrete(sid(id, TypeId::new)?))
}
fn port_key(key: &'static str) -> Result<PortKey, BuiltinAssemblyError> {
    sid(key, PortKey::new)
}
fn derived_schema(
    resolver: &'static str,
    dependencies: Vec<SchemaDependency>,
) -> Result<SchemaExpr, BuiltinAssemblyError> {
    Ok(SchemaExpr::Derived {
        resolver: sid(resolver, SchemaResolverId::new)?,
        dependencies,
    })
}
fn node_key(id: &'static str, suffix: &'static str) -> Result<I18nKey, BuiltinAssemblyError> {
    iid(leak(format!("nodes.{id}.{suffix}")))
}
fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn add_node_messages(out: &mut Vec<(&'static str, &'static str, Message)>, spec: &NodeSpec) {
    let title = leak(format!("nodes.{}.title", spec.id));
    let description = leak(format!("nodes.{}.description", spec.id));
    let documentation = leak(format!("nodes.{}.documentation", spec.id));
    let aliases = leak(format!("nodes.{}.aliases", spec.id));
    let (en_description, zh_description, en_documentation, zh_documentation) = match spec.interface
    {
        InterfaceKind::Rename => (
            "Renames one DataFrame column.",
            "重命名数据框中的一列。",
            "Renames the column identified by 'from' to 'to'.",
            "将“源列”指定的列重命名为“目标列”。",
        ),
        InterfaceKind::Project => (
            "Selects DataFrame columns in an explicit order.",
            "按明确顺序选择数据框列。",
            "Selects direct source columns without renaming or derived expressions.",
            "选择源数据框中的直接列，不执行重命名或派生表达式。",
        ),
        InterfaceKind::FilterRows => (
            "Filters DataFrame rows with a typed predicate.",
            "使用类型化谓词筛选数据框行。",
            "Filters rows by one source column and a Rust-issued compatible operator.",
            "按一个源列和 Rust 提供的兼容运算符筛选行。",
        ),
        _ => (
            "Performs a typed tabular operation.",
            "执行类型化的表格数据操作。",
            "Uses stable ports and the tabular runtime API.",
            "使用稳定端口和表格运行时 API。",
        ),
    };
    out.extend([
        ("en-US", title, Text(spec.title)),
        ("zh-CN", title, Text(spec.zh_title)),
        ("en-US", description, Text(en_description)),
        ("zh-CN", description, Text(zh_description)),
        ("en-US", documentation, Text(en_documentation)),
        ("zh-CN", documentation, Text(zh_documentation)),
        ("en-US", aliases, Aliases(spec.aliases)),
        ("zh-CN", aliases, Aliases(spec.zh_aliases)),
    ]);
}

fn add_shared_messages(out: &mut Vec<(&'static str, &'static str, Message)>) {
    for (key, en, zh) in [
        ("types.dataframe.title", "DataFrame", "数据框"),
        ("types.series.title", "DataSeries", "数据序列"),
        (
            "types.dataframe_project_columns.title",
            "DataFrame Project Columns",
            "数据框投影列",
        ),
        (
            "types.dataframe_filter_predicate.title",
            "DataFrame Filter Predicate",
            "数据框筛选谓词",
        ),
        ("categories.dataframe.title", "DataFrame", "数据框"),
        (
            "categories.dataframe.series.title",
            "DataSeries",
            "数据序列",
        ),
        (
            "categories.dataframe.timeseries.title",
            "Time Series",
            "时间序列",
        ),
        ("categories.dataframe.panel.title", "Panel Data", "面板数据"),
        (
            "editors.dataframe.connect_source",
            "Connect DataFrame input",
            "连接数据框输入",
        ),
    ] {
        out.push(("en-US", key, Text(en)));
        out.push(("zh-CN", key, Text(zh)));
    }
    for key in [
        "dataframe",
        "columns",
        "series",
        "start",
        "end",
        "step",
        "standardized",
        "mean",
        "standard_deviation",
        "time",
        "aligned",
        "entity",
    ] {
        let label = leak(format!("ports.{key}.label"));
        out.push(("en-US", label, Text(key)));
        out.push(("zh-CN", label, Text(key)));
    }
    for (key, en, zh) in [
        ("source", "Source", "源数据框"),
        ("result", "Result", "结果"),
    ] {
        let label = leak(format!("ports.{key}.label"));
        out.push(("en-US", label, Text(en)));
        out.push(("zh-CN", label, Text(zh)));
    }
    for key in [
        "dataframe",
        "column",
        "base_level",
        "frequency",
        "order",
        "window",
        "rows",
    ] {
        let title = leak(format!("parameters.{key}.title"));
        let description = leak(format!("parameters.{key}.description"));
        out.push(("en-US", title, Text(key)));
        out.push(("zh-CN", title, Text(key)));
        out.push(("en-US", description, Text("Typed node parameter.")));
        out.push(("zh-CN", description, Text("类型化节点参数。")));
    }
    for (key, en_title, zh_title, en_description, zh_description) in [
        (
            "columns",
            "Columns",
            "列",
            "Source columns to keep, in output order.",
            "按输出顺序保留的源列。",
        ),
        (
            "predicate",
            "Predicate",
            "谓词",
            "Typed condition used to keep matching rows.",
            "用于保留匹配行的类型化条件。",
        ),
        (
            "from",
            "Source column",
            "源列",
            "Column name to rename.",
            "要重命名的列名。",
        ),
        (
            "to",
            "Destination column",
            "目标列",
            "New column name.",
            "新的列名。",
        ),
    ] {
        let title = leak(format!("parameters.{key}.title"));
        let description = leak(format!("parameters.{key}.description"));
        out.push(("en-US", title, Text(en_title)));
        out.push(("zh-CN", title, Text(zh_title)));
        out.push(("en-US", description, Text(en_description)));
        out.push(("zh-CN", description, Text(zh_description)));
    }
}

#[cfg(test)]
mod tests;
