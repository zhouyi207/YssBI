//! DataFrame node protocols staged for aggregation into the built-in provider.
//!
//! This module depends exclusively on the node-system IR and keeps graph authoring,
//! pin reconciliation, and execution concerns behind their current boundaries.

use crate::builtin::node_key;
use crate::builtin::{add_described, add_node_messages, add_text, describe_parameter};
use crate::builtin::{concrete, data_port, port_key};
use yss_data_contract::DataValue;
mod aggregation;
mod families;
mod grouping;
mod inventory;
mod labels;
mod transforms;
pub(crate) use inventory::documentation as inventory_documentation;
pub(crate) use transforms::documentation as transformation_documentation;
pub(crate) fn aggregation_documentation(id: &str, locale: &str) -> Option<Box<str>> {
    if let Some(help) = grouping::documentation(id, locale) {
        return Some(help);
    }
    let kind = match id {
        "yssbi.dataframe.series.frequency" => InterfaceKind::Frequency,
        "yssbi.dataframe.groupby" => InterfaceKind::GroupBy,
        _ => return None,
    };
    let (en, zh) = aggregation::help(kind);
    Some(
        if crate::documentation::is_chinese_locale(locale) {
            zh
        } else {
            en
        }
        .into(),
    )
}

use super::builtin::{
    BuiltinAssemblyError, ProviderFragment, assembled_interface, assembled_parameters, iid, leaf,
    pure, sid,
};
use crate::Message;
use yss_node_protocol::*;
use yss_node_registry::{CategoryRegistration, RegisteredNode, TypeRegistration};

use families::{InterfaceKind, NODES, NodeSpec};

pub const DATAFRAME_DROP_NA_SCHEMA_RESOLVER: &str = "yssbi.dataframe.schema.dropna";

pub const DATAFRAME_COLUMNS_RESOLVER: &str = "yssbi.dataframe.interface.columns";
pub const DATAFRAME_RESOURCE_SCHEMA_RESOLVER: &str = "yssbi.dataframe.schema.resource";
pub const DATAFRAME_COMPOSITION_SCHEMA_RESOLVER: &str = "yssbi.dataframe.schema.composition";

pub(crate) fn build_provider_fragment() -> Result<ProviderFragment, BuiltinAssemblyError> {
    let mut messages = Vec::new();
    add_shared_messages(&mut messages);
    let nodes = NODES
        .iter()
        .map(|spec| {
            add_node_messages(
                &mut messages,
                spec.id,
                [spec.title, spec.zh_title],
                Some([spec.aliases, spec.zh_aliases]),
            );
            registered_node(spec)
        })
        .collect::<Result<Vec<_>, BuiltinAssemblyError>>()?;

    let mut fragment = ProviderFragment {
        types: dataframe_types()?,
        categories: dataframe_categories()?,
        interface_resolvers: vec![sid(DATAFRAME_COLUMNS_RESOLVER, InterfaceResolverId::new)?],
        schema_resolvers: vec![
            sid(DATAFRAME_DROP_NA_SCHEMA_RESOLVER, SchemaResolverId::new)?,
            sid(DATAFRAME_RESOURCE_SCHEMA_RESOLVER, SchemaResolverId::new)?,
            sid(DATAFRAME_COMPOSITION_SCHEMA_RESOLVER, SchemaResolverId::new)?,
            sid("yssbi.dataframe.schema.aggregate", SchemaResolverId::new)?,
        ],
        nodes,
        messages,
        ..ProviderFragment::default()
    };
    inventory::append(&mut fragment)?;
    transforms::append(&mut fragment)?;
    labels::append(&mut fragment)?;
    grouping::append(&mut fragment)?;
    Ok(fragment)
}

fn registered_node(spec: &NodeSpec) -> Result<RegisteredNode, BuiltinAssemblyError> {
    let protocol = protocol(spec)?;
    Ok(leaf(protocol, spec.id))
}

fn protocol(spec: &NodeSpec) -> Result<NodeProtocol, BuiltinAssemblyError> {
    let (ports, parameters) = interface(spec.interface)?;
    let type_parameters = match spec.interface {
        InterfaceKind::SeriesSelect
        | InterfaceKind::SeriesLength
        | InterfaceKind::SeriesCount
        | InterfaceKind::TimeLag => vec![sid("element", TypeParameterId::new)?],
        _ => vec![],
    };
    Ok(NodeProtocol {
        type_id: sid(spec.id, NodeTypeId::new)?,
        catalog: NodeCatalogProtocol {
            title_key: node_key(spec.id, "title")?,
            aliases_key: Some(node_key(spec.id, "aliases")?),
            category_id: sid(category(spec.interface), NodeCategoryId::new)?,
            icon_id: sid("builtin.dataframe", IconId::new)?,
            style_id: sid("builtin.dataframe", NodeStyleId::new)?,
            hidden: false,
        },
        interface: assembled_interface(spec.id, ports, type_parameters, vec![])?,
        parameters: assembled_parameters(spec.id, parameters)?,
        instance_display: match spec.interface {
            InterfaceKind::DataframeSource => NodeInstanceDisplaySpec::ResourceParameter {
                parameter: sid("dataframe", ParameterKey::new)?,
                kind: ResourceDisplayKind::Database,
            },
            _ => NodeInstanceDisplaySpec::Static,
        },
        execution: pure(),
        typing: if spec.interface == InterfaceKind::SeriesSelect {
            NodeTypingSpec::ColumnOutput {
                input: sid("dataframe", PortKey::new)?,
                column: sid("column", ParameterKey::new)?,
                output: sid("series", PortKey::new)?,
            }
        } else if spec.interface == InterfaceKind::DummyInfo {
            NodeTypingSpec::Identity {
                input: sid("source", PortKey::new)?,
                output: sid("result", PortKey::new)?,
            }
        } else {
            NodeTypingSpec::Fixed
        },
        scope: NodeScope::Any,
        managed_role: None,
    })
}

fn interface(kind: InterfaceKind) -> Result<(Vec<PortSpec>, Vec<Parameter>), BuiltinAssemblyError> {
    use InterfaceKind::*;
    match kind {
        Frequency | GroupBy => aggregation::interface(kind),
        DataframeSource => Ok((
            vec![data_output(
                "dataframe",
                "DataFrame",
                dataframe_type()?,
                Some(derived_schema(DATAFRAME_RESOURCE_SCHEMA_RESOLVER, vec![])?),
            )?],
            vec![resource_parameter("dataframe")?],
        )),
        Limit => Ok((
            vec![
                data_input("source", "Source", dataframe_type()?)?,
                data_output(
                    "result",
                    "Result",
                    dataframe_type()?,
                    Some(SchemaExpr::Input(port_key("source")?)),
                )?,
            ],
            vec![
                bounded_positive_integer_parameter("rows", 100, 1_000_000)?,
                transforms::offset_parameter()?,
            ],
        )),
        Rename => Ok((
            vec![
                data_input("source", "Source", dataframe_type()?)?,
                data_output(
                    "result",
                    "Result",
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
            vec![column_parameter("from")?, required_column_name("to")?],
        )),
        DropNaRows | DropNaColumns => {
            let subset_type = data_series_type(concrete("core.text")?);
            Ok((
                vec![
                    data_input("source", "Source", dataframe_type()?)?,
                    data_output(
                        "result",
                        "Result",
                        dataframe_type()?,
                        Some(if kind == DropNaRows {
                            SchemaExpr::Input(port_key("source")?)
                        } else {
                            derived_schema(
                                DATAFRAME_DROP_NA_SCHEMA_RESOLVER,
                                vec![SchemaDependency::Port(port_key("source")?)],
                            )?
                        }),
                    )?,
                ],
                vec![
                    parameter(
                        "subset",
                        subset_type.clone(),
                        ParameterEditorSpec::Auto,
                        Some(TypedValue {
                            value_type: subset_type,
                            value: DataValue::List(vec![]),
                        }),
                        vec![ParameterConstraint::ColumnNames],
                    )?,
                    choice_parameter(
                        "how",
                        if kind == DropNaRows { "any" } else { "all" },
                        &["any", "all"],
                    )?,
                ],
            ))
        }
        Project | DropColumns => Ok((
            relational_ports(SchemaExpr::Project {
                input: Box::new(SchemaExpr::Input(port_key("source")?)),
                columns: if kind == DropColumns {
                    ColumnSelectionExpr::ExcludingParameter(sid("columns", ParameterKey::new)?)
                } else {
                    ColumnSelectionExpr::FromParameter(sid("columns", ParameterKey::new)?)
                },
            })?,
            vec![nominal_parameter(
                "columns",
                yss_node_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
            )?],
        )),
        FilterRows | DropRows => Ok((
            relational_ports(SchemaExpr::Filter {
                input: Box::new(SchemaExpr::Input(port_key("source")?)),
                predicate: Some(sid("predicate", ParameterKey::new)?),
            })?,
            vec![nominal_parameter(
                "predicate",
                yss_node_protocol::dataframe::FILTER_PREDICATE_TYPE_ID,
            )?],
        )),
        Decompose => Ok((
            vec![
                data_input("dataframe", "DataFrame", dataframe_type()?)?,
                derived_output(
                    "columns",
                    "Column",
                    series_type()?,
                    DATAFRAME_COLUMNS_RESOLVER,
                )?,
            ],
            vec![],
        )),
        Combine => Ok((
            vec![
                user_input("series", "DataSeries", series_type()?, 1)?,
                data_output(
                    "dataframe",
                    "DataFrame",
                    dataframe_type()?,
                    Some(derived_schema(
                        DATAFRAME_COMPOSITION_SCHEMA_RESOLVER,
                        vec![SchemaDependency::Port(port_key("series")?)],
                    )?),
                )?,
            ],
            vec![],
        )),
        ConcatRows | ConcatColumns => Ok((
            vec![
                user_input("frames", "DataFrame", dataframe_type()?, 2)?,
                data_output(
                    "result",
                    "Result",
                    dataframe_type()?,
                    Some(derived_schema(
                        DATAFRAME_COMPOSITION_SCHEMA_RESOLVER,
                        vec![SchemaDependency::Port(port_key("frames")?)],
                    )?),
                )?,
            ],
            if kind == ConcatRows {
                vec![choice_parameter(
                    "column_match",
                    "by_name",
                    &["by_name", "by_position"],
                )?]
            } else {
                vec![]
            },
        )),
        Join => Ok((
            vec![
                data_input("left", "Left", dataframe_type()?)?,
                data_input("right", "Right", dataframe_type()?)?,
                data_output(
                    "result",
                    "Result",
                    dataframe_type()?,
                    Some(derived_schema(
                        DATAFRAME_COMPOSITION_SCHEMA_RESOLVER,
                        vec![
                            SchemaDependency::Port(port_key("left")?),
                            SchemaDependency::Port(port_key("right")?),
                        ],
                    )?),
                )?,
            ],
            vec![
                nominal_parameter(
                    "left_keys",
                    yss_node_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
                )?,
                nominal_parameter(
                    "right_keys",
                    yss_node_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
                )?,
                choice_parameter(
                    "join_type",
                    "inner",
                    &["inner", "left", "right", "full", "semi", "anti"],
                )?,
                parameter(
                    "right_suffix",
                    concrete("core.text")?,
                    ParameterEditorSpec::Text { multiline: false },
                    Some(TypedValue {
                        value_type: concrete("core.text")?,
                        value: DataValue::String("_right".into()),
                    }),
                    vec![ParameterConstraint::Length {
                        min: Some(1),
                        max: Some(128),
                    }],
                )?,
            ],
        )),
        SeriesSelect => Ok((
            vec![
                data_input("dataframe", "DataFrame", dataframe_type()?)?,
                data_output(
                    "series",
                    "DataSeries",
                    generic_series_type("element")?,
                    None,
                )?,
            ],
            vec![column_parameter("column")?],
        )),
        IntRange => Ok((
            vec![data_output(
                "series",
                "DataSeries",
                numeric_data_series_type(),
                None,
            )?],
            [("start", 0), ("end", 10), ("step", 1)]
                .into_iter()
                .map(|(key, default)| {
                    parameter(
                        key,
                        concrete("core.numeric")?,
                        ParameterEditorSpec::Number,
                        Some(TypedValue {
                            value_type: concrete("core.numeric")?,
                            value: DataValue::Integer(default),
                        }),
                        vec![ParameterConstraint::IntegerRange {
                            min: None,
                            max: None,
                        }],
                    )
                })
                .collect::<Result<_, BuiltinAssemblyError>>()?,
        )),
        SeriesLength | SeriesCount => Ok((
            vec![
                data_input("series", "DataSeries", generic_series_type("element")?)?,
                data_output("value", "Value", concrete("core.numeric")?, None)?,
            ],
            vec![],
        )),
        SeriesSum => Ok((
            vec![
                data_input("series", "DataSeries", numeric_data_series_type())?,
                data_output("value", "Value", concrete("core.numeric")?, None)?,
            ],
            vec![],
        )),
        SeriesMean => Ok((
            vec![
                data_input("series", "DataSeries", numeric_data_series_type())?,
                data_output("value", "Value", float_type()?, None)?,
            ],
            vec![],
        )),
        Standardize => Ok((
            vec![
                data_input("series", "DataSeries", numeric_data_series_type())?,
                data_output(
                    "standardized",
                    "Standardized",
                    numeric_data_series_type(),
                    None,
                )?,
                data_output("mean", "Mean", float_type()?, None)?,
                data_output(
                    "standard_deviation",
                    "Standard Deviation",
                    float_type()?,
                    None,
                )?,
            ],
            vec![],
        )),
        InverseStandardize => Ok((
            vec![
                data_input("standardized", "Standardized", numeric_data_series_type())?,
                scalar_input("mean", "Mean", "core.numeric")?,
                scalar_input("standard_deviation", "Standard Deviation", "core.numeric")?,
                data_output("series", "DataSeries", numeric_data_series_type(), None)?,
            ],
            vec![],
        )),
        DummyInfo => {
            let series = TypeExpr::Union(
                ["core.categorical", "core.ordinal", "core.binary"]
                    .into_iter()
                    .map(|id| concrete(id).map(data_series_type))
                    .collect::<Result<_, _>>()?,
            );
            Ok((
                vec![
                    data_input("source", "Source", series.clone())?,
                    data_output("result", "Result", series, None)?,
                ],
                vec![parameter(
                    "base_level",
                    concrete("core.text")?,
                    ParameterEditorSpec::Text { multiline: false },
                    Some(TypedValue {
                        value_type: concrete("core.text")?,
                        value: DataValue::String("".into()),
                    }),
                    vec![],
                )?],
            ))
        }
        TimeAlign => Ok((
            vec![
                data_input("dataframe", "DataFrame", dataframe_type()?)?,
                data_output(
                    "aligned",
                    "Aligned",
                    dataframe_type()?,
                    Some(SchemaExpr::Input(port_key("dataframe")?)),
                )?,
            ],
            vec![
                column_parameter("time_column")?,
                positive_integer_parameter("interval", 1)?,
            ],
        )),
        TimeUnary => Ok((
            vec![
                data_input("series", "DataSeries", numeric_data_series_type())?,
                data_output("result", "Result", numeric_data_series_type(), None)?,
            ],
            vec![positive_integer_parameter("order", 1)?],
        )),
        TimeWindow => Ok((
            vec![
                data_input("series", "DataSeries", numeric_data_series_type())?,
                transforms::window_context()?,
                data_output("result", "Result", numeric_data_series_type(), None)?,
            ],
            std::iter::once(positive_integer_parameter("window", 1)?)
                .chain([
                    choice_parameter("operation", "mean", &["mean", "sum", "min", "max", "std"])?,
                    transforms::minimum_periods_parameter()?,
                ])
                .chain(transforms::window_parameters()?)
                .collect(),
        )),
        TimeLag => Ok((
            vec![
                data_input("series", "DataSeries", generic_series_type("element")?)?,
                transforms::window_context()?,
                data_output("result", "Result", generic_series_type("element")?, None)?,
            ],
            std::iter::once(positive_integer_parameter("window", 1)?)
                .chain([choice_parameter("direction", "lag", &["lag", "lead"])?])
                .chain(transforms::window_parameters()?)
                .collect(),
        )),
        PanelAlign => Ok((
            vec![
                data_input("dataframe", "DataFrame", dataframe_type()?)?,
                data_output(
                    "aligned",
                    "Aligned",
                    dataframe_type()?,
                    Some(SchemaExpr::Input(port_key("dataframe")?)),
                )?,
            ],
            vec![
                column_parameter("entity_column")?,
                column_parameter("time_column")?,
                positive_integer_parameter("interval", 1)?,
            ],
        )),
        PanelDifference => Ok((
            vec![
                data_input("aligned", "Aligned", dataframe_type()?)?,
                data_input("series", "DataSeries", numeric_data_series_type())?,
                data_output("result", "Result", numeric_data_series_type(), None)?,
            ],
            vec![
                positive_integer_parameter("order", 1)?,
                column_parameter("entity_column")?,
                column_parameter("time_column")?,
            ],
        )),
    }
}

fn choice_parameter(
    key: &'static str,
    default: &'static str,
    values: &[&str],
) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.text")?,
        ParameterEditorSpec::Select,
        Some(TypedValue {
            value_type: concrete("core.text")?,
            value: DataValue::String(default.into()),
        }),
        vec![ParameterConstraint::OneOf(
            values
                .iter()
                .map(|v| DataValue::String((*v).into()))
                .collect(),
        )],
    )
}

fn relational_ports(result_schema: SchemaExpr) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    Ok(vec![
        data_input("source", "Source", dataframe_type()?)?,
        data_output("result", "Result", dataframe_type()?, Some(result_schema))?,
    ])
}

fn data_input(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port(
        key,
        title,
        PortDirection::Input,
        value_type,
        PortCardinality::Declared,
        LiteralPolicy::Forbidden,
    )
}

fn scalar_input(
    key: &'static str,
    title: &'static str,
    type_id: &'static str,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_input(key, title, concrete(type_id)?)
}

fn user_input(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
    min: u16,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port(
        key,
        title,
        PortDirection::Input,
        value_type,
        PortCardinality::UserCreated { min, max: None },
        LiteralPolicy::Forbidden,
    )
}

fn data_output(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
    schema: Option<SchemaExpr>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        schema,
        ..data_port(
            key,
            title,
            PortDirection::Output,
            value_type,
            PortCardinality::Declared,
            LiteralPolicy::Forbidden,
        )?
    })
}

fn derived_output(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
    resolver: &'static str,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_port(
        key,
        title,
        PortDirection::Output,
        value_type,
        PortCardinality::Derived {
            resolver: sid(resolver, InterfaceResolverId::new)?,
        },
        LiteralPolicy::Forbidden,
    )
}

fn resource_parameter(key: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.text")?,
        ParameterEditorSpec::Resource {
            kind: ResourceDisplayKind::Database,
        },
        None,
        vec![ParameterConstraint::Required],
    )
}

fn column_parameter(key: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.text")?,
        ParameterEditorSpec::Select,
        None,
        vec![
            ParameterConstraint::Required,
            ParameterConstraint::ColumnName,
        ],
    )
}

fn required_column_name(key: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    let mut parameter = required_text_parameter(key)?;
    parameter.constraints.push(ParameterConstraint::ColumnName);
    Ok(parameter)
}

fn nominal_parameter(
    key: &'static str,
    type_id: &'static str,
) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete(type_id)?,
        ParameterEditorSpec::Auto,
        None,
        vec![ParameterConstraint::Required],
    )
}

fn required_text_parameter(key: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.text")?,
        ParameterEditorSpec::Text { multiline: false },
        None,
        vec![ParameterConstraint::Required],
    )
}

fn positive_integer_parameter(
    key: &'static str,
    default: i64,
) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.numeric")?,
        ParameterEditorSpec::Number,
        Some(TypedValue {
            value_type: concrete("core.numeric")?,
            value: DataValue::Integer(default),
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
) -> Result<Parameter, BuiltinAssemblyError> {
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
    default_value: Option<TypedValue>,
    constraints: Vec<ParameterConstraint>,
) -> Result<Parameter, BuiltinAssemblyError> {
    Ok(Parameter {
        key: sid(key, ParameterKey::new)?,
        title_key: iid(format!("parameters.{key}.title"))?,
        description_key: Some(iid(format!("parameters.{key}.description"))?),
        value_type,
        default_value,
        constraints,
        editor,
        presentation: ParameterPresentation::DetailPanel,
        visible_when: None,
    })
}

fn dataframe_types() -> Result<Vec<TypeRegistration>, BuiltinAssemblyError> {
    [
        ("tabular.dataframe", "types.dataframe.title"),
        (
            yss_node_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
            "types.dataframe_project_columns.title",
        ),
        (
            yss_node_protocol::dataframe::FILTER_PREDICATE_TYPE_ID,
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
        ("database", Some("data"), 55),
        ("data_processing", None, 60),
        ("dataframe", Some("data_processing"), 60),
        ("dataframe.series", Some("data_processing"), 61),
    ]
    .into_iter()
    .map(|(id, parent, order)| {
        Ok(CategoryRegistration {
            id: sid(id, NodeCategoryId::new)?,
            title_key: iid(format!("categories.{id}.title"))?,
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
        InterfaceKind::DataframeSource => "database",
        InterfaceKind::Limit
        | InterfaceKind::Rename
        | InterfaceKind::Project
        | InterfaceKind::FilterRows
        | InterfaceKind::DropColumns
        | InterfaceKind::DropRows
        | InterfaceKind::DropNaRows
        | InterfaceKind::DropNaColumns
        | InterfaceKind::Decompose
        | InterfaceKind::Combine
        | InterfaceKind::ConcatRows
        | InterfaceKind::ConcatColumns
        | InterfaceKind::Join
        | InterfaceKind::GroupBy
        | InterfaceKind::TimeAlign
        | InterfaceKind::PanelAlign => "dataframe",
        InterfaceKind::SeriesSelect
        | InterfaceKind::IntRange
        | InterfaceKind::SeriesLength
        | InterfaceKind::SeriesCount
        | InterfaceKind::SeriesSum
        | InterfaceKind::SeriesMean
        | InterfaceKind::Frequency
        | InterfaceKind::Standardize
        | InterfaceKind::InverseStandardize
        | InterfaceKind::DummyInfo
        | InterfaceKind::TimeUnary
        | InterfaceKind::TimeWindow
        | InterfaceKind::TimeLag
        | InterfaceKind::PanelDifference => "dataframe.series",
    }
}

fn dataframe_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete("tabular.dataframe")
}
fn series_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(data_series_type(TypeExpr::Unknown))
}
fn generic_series_type(id: &'static str) -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(data_series_type(TypeExpr::Generic(sid(
        id,
        TypeParameterId::new,
    )?)))
}
fn float_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete("core.numeric")
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

fn add_shared_messages(out: &mut Vec<(&'static str, String, Message)>) {
    aggregation::messages(out);
    transforms::shared_window_messages(out);
    for (key, en, zh) in [
        ("types.dataframe.title", "DataFrame", "数据框"),
        ("types.series.title", "DataSeries", "数据序列"),
        (
            "types.dataframe_project_columns.title",
            "Selected Columns",
            "筛选列清单",
        ),
        (
            "types.dataframe_filter_predicate.title",
            "DataFrame Filter Predicate",
            "数据框筛选谓词",
        ),
        ("categories.database.title", "Database", "数据库"),
        (
            "categories.data_processing.title",
            "Data Processing",
            "数据处理",
        ),
        ("categories.dataframe.title", "DataFrame", "数据帧"),
        (
            "categories.dataframe.series.title",
            "DataSeries",
            "数据序列",
        ),
        (
            "editors.dataframe.connect_source",
            "Enter column names now; they will be checked when the input schema is known.",
            "可先填写列名，输入结构确定后会校验这些列。",
        ),
        (
            "editors.dataframe.deferred_columns",
            "Column membership depends on execution; the configured names are pending validation.",
            "列结构取决于执行结果；已配置的列名等待校验。",
        ),
        (
            "editors.dataframe.pending_columns",
            "The upstream schema is unresolved; you can continue configuring column names.",
            "上游结构尚未确定，可以继续配置列名。",
        ),
        (
            "editors.dataframe.schema_error",
            "The input schema has a problem. Keep editing parameters and check the graph diagnostics.",
            "输入结构存在问题。参数仍可编辑，请查看图诊断。",
        ),
    ] {
        add_text(out, key, [en, zh]);
    }
    for key in [
        "dataframe",
        "column",
        "base_level",
        "order",
        "window",
        "rows",
    ] {
        add_described(
            out,
            &format!("parameters.{key}"),
            [key, key],
            ["Typed node parameter.", "类型化节点参数。"],
        );
    }
    for (key, en_title, zh_title, en_description, zh_description) in [
        (
            "interval",
            "Grid interval",
            "网格间隔",
            "Positive step: time units for time-series alignment; shared observed-time positions for panel alignment.",
            "正整数步长：时间序列按时间单位，面板按共享已观测时间序列的位置补齐。",
        ),
        (
            "entity_column",
            "Entity Column",
            "实体列",
            "Groups panel alignment or differences by this column; missing keys are rejected.",
            "按此列分组对齐面板或计算差分，键不允许缺失。",
        ),
        (
            "time_column",
            "Time Column",
            "时间列",
            "Orders rows within each entity; duplicate entity-time keys are rejected.",
            "按此列在实体内排序，实体与时间组合不允许重复。",
        ),
        (
            "subset",
            "Columns to Check (empty means all)",
            "检查列（不选表示全部）",
            "Empty checks all columns; otherwise supply unique column names.",
            "空列表检查全部列；也可填写不重复的列名。",
        ),
        (
            "how",
            "Missing Rule",
            "缺失规则",
            "any: at least one Null; all: every value is Null.",
            "any：任一值为 Null；all：全部值为 Null。",
        ),
        (
            "column_match",
            "Column Matching",
            "列对齐方式",
            "Match columns by name (fill missing columns with null) or by position.",
            "按列名对齐并补齐缺失列，或按列位置对齐。",
        ),
        (
            "left_keys",
            "Left Keys",
            "左表连接键",
            "Select the left key columns in matching order.",
            "按匹配顺序选择左表连接列。",
        ),
        (
            "right_keys",
            "Right Keys",
            "右表连接键",
            "Select the corresponding right key columns.",
            "按相同顺序选择右表连接列。",
        ),
        (
            "join_type",
            "Join Type",
            "连接类型",
            "Inner, left, right, full outer, semi or anti join. Null keys do not match.",
            "选择内连接、左连接、右连接、全外连接、半连接或反连接；空键不匹配。",
        ),
        (
            "right_suffix",
            "Right Column Suffix",
            "右表重名列后缀",
            "Suffix used for duplicate right column names.",
            "右表重名列自动添加的后缀。",
        ),
        (
            "start",
            "Start",
            "起点",
            "First value in the integer range.",
            "整数范围的起始值。",
        ),
        (
            "end",
            "End",
            "终点",
            "Exclusive end of the integer range.",
            "整数范围的终止值（不包含）。",
        ),
        (
            "step",
            "Step",
            "步长",
            "Increment between consecutive values.",
            "相邻整数之间的增量。",
        ),
        (
            "columns",
            "Columns",
            "列",
            "Source columns selected for this operation.",
            "选择本次操作使用的源列。",
        ),
        (
            "predicate",
            "Predicate",
            "谓词",
            "Typed condition identifying matching rows.",
            "用于确定匹配行的类型化条件。",
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
        add_described(
            out,
            &format!("parameters.{key}"),
            [en_title, zh_title],
            [en_description, zh_description],
        );
    }
}
