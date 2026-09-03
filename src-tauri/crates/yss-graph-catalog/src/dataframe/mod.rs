//! DataFrame node protocols staged for aggregation into the built-in provider.
//!
//! This module depends exclusively on the node-system IR and keeps graph authoring,
//! pin reconciliation, and execution concerns behind their current boundaries.

mod families;

use super::builtin::{
    BuiltinAssemblyError, ProviderFragment, assembled_interface, assembled_parameters, iid, leaf,
    sid,
};
use crate::{Aliases, Message, Text};
use yss_graph_protocol::*;
use yss_graph_registry::{CategoryRegistration, RegisteredNode, TypeRegistration};

use families::{InterfaceKind, NODES, NodeSpec};

pub const DATAFRAME_COLUMNS_RESOLVER: &str = "yssbi.dataframe.interface.columns";
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
    Ok(leaf(protocol, spec.kernel))
}

fn protocol(spec: &NodeSpec) -> Result<NodeProtocol, BuiltinAssemblyError> {
    let (ports, parameters) = interface(spec.interface)?;
    let type_parameters = match spec.interface {
        InterfaceKind::Combine
        | InterfaceKind::SeriesSelect
        | InterfaceKind::SeriesLength
        | InterfaceKind::SeriesCount
        | InterfaceKind::DummyInfo
        | InterfaceKind::TimeLag => vec![sid("element", TypeParameterId::new)?],
        InterfaceKind::TimeAlign => vec![sid("time", TypeParameterId::new)?],
        InterfaceKind::PanelAlign => vec![
            sid("entity", TypeParameterId::new)?,
            sid("time", TypeParameterId::new)?,
        ],
        _ => vec![],
    };
    Ok(NodeProtocol {
        type_id: sid(spec.id, NodeTypeId::new)?,
        catalog: NodeCatalogProtocol {
            title_key: node_key(spec.id, "title")?,
            documentation_key: Some(node_key(spec.id, "documentation")?),
            aliases_key: Some(node_key(spec.id, "aliases")?),
            category_id: sid(category(spec.interface), NodeCategoryId::new)?,
            icon_id: sid("builtin.dataframe", IconId::new)?,
            style_id: sid("builtin.dataframe", NodeStyleId::new)?,
            hidden: false,
        },
        interface: assembled_interface(spec.id, ports, type_parameters, vec![], vec![])?,
        parameters: assembled_parameters(spec.id, parameters)?,
        instance_display: match spec.interface {
            InterfaceKind::DataframeSource => NodeInstanceDisplaySpec::ResourceParameter {
                parameter: sid("dataframe", ParameterKey::new)?,
                kind: ResourceDisplayKind::Database,
            },
            _ => NodeInstanceDisplaySpec::Static,
        },
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            cache: CachePolicy::PerRun,
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
                "DataFrame",
                dataframe_type()?,
                Some(derived_schema(DATAFRAME_RESOURCE_SCHEMA_RESOLVER, vec![])?),
            )?],
            vec![resource_parameter("dataframe")?],
        )),
        Limit => Ok((
            vec![
                streaming_input("source", "Source", dataframe_type()?, None)?,
                streaming_output(
                    "result",
                    "Result",
                    dataframe_type()?,
                    Some(SchemaExpr::Input(port_key("source")?)),
                )?,
            ],
            vec![bounded_positive_integer_parameter("rows", 100, 1_000_000)?],
        )),
        Rename => Ok((
            vec![
                streaming_input("source", "Source", dataframe_type()?, None)?,
                streaming_output(
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
                yss_graph_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
            )?],
        )),
        FilterRows => Ok((
            relational_ports(SchemaExpr::Filter {
                input: Box::new(SchemaExpr::Input(port_key("source")?)),
                predicate: Some(sid("predicate", ParameterKey::new)?),
            })?,
            vec![nominal_parameter(
                "predicate",
                yss_graph_protocol::dataframe::FILTER_PREDICATE_TYPE_ID,
            )?],
        )),
        Decompose => Ok((
            vec![
                data_input("dataframe", "DataFrame", dataframe_type()?, None)?,
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
                user_input("series", "DataSeries", generic_series_type("element")?, 1)?,
                data_output(
                    "dataframe",
                    "DataFrame",
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
                data_input("source", "Source", dataframe_type()?, None)?,
                data_input("condition", "Condition", bool_series_type()?, None)?,
                data_output(
                    "result",
                    "Result",
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
                data_input("dataframe", "DataFrame", dataframe_type()?, None)?,
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
            vec![
                scalar_input("start", "Start", "core.int64")?,
                scalar_input("end", "End", "core.int64")?,
                scalar_input("step", "Step", "core.int64")?,
                data_output("series", "DataSeries", int_series_type()?, None)?,
            ],
            vec![],
        )),
        SeriesLength | SeriesCount => Ok((
            vec![
                data_input(
                    "series",
                    "DataSeries",
                    generic_series_type("element")?,
                    None,
                )?,
                data_output("value", "Value", concrete("core.int64")?, None)?,
            ],
            vec![],
        )),
        SeriesSum => Ok((
            vec![
                data_input("series", "DataSeries", numeric_series_type(), None)?,
                data_output("value", "Value", numeric_scalar_type()?, None)?,
            ],
            vec![],
        )),
        SeriesMean => Ok((
            vec![
                data_input("series", "DataSeries", numeric_series_type(), None)?,
                data_output("value", "Value", float_type()?, None)?,
            ],
            vec![],
        )),
        NumericCompare => Ok((
            vec![
                data_input("left", "Left", numeric_series_type(), None)?,
                data_input("right", "Right", numeric_series_or_scalar_type()?, None)?,
                data_output("result", "Result", bool_series_type()?, None)?,
            ],
            vec![],
        )),
        StringCompare => Ok((
            vec![
                data_input("left", "Left", string_series_type()?, None)?,
                data_input("right", "Right", string_series_or_scalar_type()?, None)?,
                data_output("result", "Result", bool_series_type()?, None)?,
            ],
            vec![],
        )),
        Standardize => Ok((
            vec![
                data_input("series", "DataSeries", numeric_series_type(), None)?,
                data_output("standardized", "Standardized", float_series_type()?, None)?,
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
                data_input("standardized", "Standardized", float_series_type()?, None)?,
                scalar_input("mean", "Mean", "core.float64")?,
                scalar_input("standard_deviation", "Standard Deviation", "core.float64")?,
                data_output("series", "DataSeries", float_series_type()?, None)?,
            ],
            vec![],
        )),
        DummyInfo => Ok((
            vec![
                data_input("source", "Source", generic_series_type("element")?, None)?,
                data_output("result", "Result", generic_series_type("element")?, None)?,
            ],
            vec![text_parameter("base_level", false)?],
        )),
        TimeAlign => Ok((
            vec![
                data_input("dataframe", "DataFrame", dataframe_type()?, None)?,
                data_input("time", "Time", generic_series_type("time")?, None)?,
                data_output(
                    "aligned",
                    "Aligned",
                    dataframe_type()?,
                    Some(SchemaExpr::Input(port_key("dataframe")?)),
                )?,
            ],
            vec![select_parameter("frequency")?],
        )),
        TimeUnary => Ok((
            vec![
                data_input("series", "DataSeries", numeric_series_type(), None)?,
                data_output("result", "Result", float_series_type()?, None)?,
            ],
            vec![positive_integer_parameter("order", 1)?],
        )),
        TimeWindow => Ok((
            vec![
                data_input("series", "DataSeries", numeric_series_type(), None)?,
                data_output("result", "Result", float_series_type()?, None)?,
            ],
            vec![positive_integer_parameter("window", 1)?],
        )),
        TimeLag => Ok((
            vec![
                data_input(
                    "series",
                    "DataSeries",
                    generic_series_type("element")?,
                    None,
                )?,
                data_output("result", "Result", generic_series_type("element")?, None)?,
            ],
            vec![positive_integer_parameter("window", 1)?],
        )),
        PanelAlign => Ok((
            vec![
                data_input("dataframe", "DataFrame", dataframe_type()?, None)?,
                data_input("entity", "Entity", generic_series_type("entity")?, None)?,
                data_input("time", "Time", generic_series_type("time")?, None)?,
                data_output(
                    "aligned",
                    "Aligned",
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
                data_input("aligned", "Aligned", dataframe_type()?, None)?,
                data_input("series", "DataSeries", numeric_series_type(), None)?,
                data_output("result", "Result", float_series_type()?, None)?,
            ],
            vec![positive_integer_parameter("order", 1)?],
        )),
    }
}

fn relational_ports(result_schema: SchemaExpr) -> Result<Vec<PortSpec>, BuiltinAssemblyError> {
    Ok(vec![
        streaming_input("source", "Source", dataframe_type()?, None)?,
        streaming_output("result", "Result", dataframe_type()?, Some(result_schema))?,
    ])
}

fn data_input(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
    schema: Option<SchemaExpr>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(
        key,
        title,
        PortDirection::Input,
        value_type,
        PortInstances::Declared,
        schema,
    )
}

fn streaming_input(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
    schema: Option<SchemaExpr>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    let mut spec = data_input(key, title, value_type, schema)?;
    spec.consumption = Some(InputConsumption::Streaming);
    Ok(spec)
}

fn scalar_input(
    key: &'static str,
    title: &'static str,
    type_id: &'static str,
) -> Result<PortSpec, BuiltinAssemblyError> {
    data_input(key, title, concrete(type_id)?, None)
}

fn user_input(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
    min: u16,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(
        key,
        title,
        PortDirection::Input,
        value_type,
        PortInstances::UserCreated { min, max: None },
        None,
    )
}

fn data_output(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
    schema: Option<SchemaExpr>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(
        key,
        title,
        PortDirection::Output,
        value_type,
        PortInstances::Declared,
        schema,
    )
}

fn streaming_output(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
    schema: Option<SchemaExpr>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    let mut spec = data_output(key, title, value_type, schema)?;
    spec.production = Some(OutputProduction::Streaming);
    Ok(spec)
}

fn derived_output(
    key: &'static str,
    title: &'static str,
    value_type: TypeExpr,
    resolver: &'static str,
) -> Result<PortSpec, BuiltinAssemblyError> {
    port(
        key,
        title,
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
    title: &'static str,
    direction: PortDirection,
    value_type: TypeExpr,
    instances: PortInstances,
    schema: Option<SchemaExpr>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    Ok(PortSpec {
        key: port_key(key)?,
        title: title.into(),
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
        ParameterEditorSpec::Resource {
            kind: ResourceDisplayKind::Database,
        },
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
        presentation: ParameterPresentation::DetailPanel,
    })
}

fn dataframe_types() -> Result<Vec<TypeRegistration>, BuiltinAssemblyError> {
    [
        ("tabular.dataframe", "types.dataframe.title"),
        (
            yss_graph_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
            "types.dataframe_project_columns.title",
        ),
        (
            yss_graph_protocol::dataframe::FILTER_PREDICATE_TYPE_ID,
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
fn numeric_series_type() -> TypeExpr {
    numeric_data_series_type()
}
fn float_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    concrete("core.float64")
}
fn numeric_scalar_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    normalized_union(
        "dataframe numeric scalar union",
        vec![concrete("core.int64")?, concrete("core.float64")?],
    )
}
fn int_series_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(data_series_type(concrete("core.int64")?))
}
fn float_series_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(data_series_type(concrete("core.float64")?))
}
fn string_series_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(data_series_type(concrete("core.string")?))
}
fn bool_series_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(data_series_type(concrete("core.bool")?))
}
fn numeric_series_or_scalar_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    normalized_union(
        "dataframe numeric series/scalar union",
        vec![numeric_series_type(), numeric_scalar_type()?],
    )
}
fn string_series_or_scalar_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    normalized_union(
        "dataframe string series/scalar union",
        vec![string_series_type()?, concrete("core.string")?],
    )
}
fn normalized_union(
    context: &'static str,
    members: Vec<TypeExpr>,
) -> Result<TypeExpr, BuiltinAssemblyError> {
    normalize_type_expr(TypeExpr::Union(members)).map_err(|error| {
        BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
            context,
            value: error.to_string().into(),
        }
    })
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
    let documentation = leak(format!("nodes.{}.documentation", spec.id));
    let aliases = leak(format!("nodes.{}.aliases", spec.id));
    let (en_documentation, zh_documentation) = match spec.interface {
        InterfaceKind::Rename => (
            "Renames the column identified by 'from' to 'to'.",
            "将“源列”指定的列重命名为“目标列”。",
        ),
        InterfaceKind::Project => (
            "Selects direct source columns without renaming or derived expressions.",
            "选择源数据框中的直接列，不执行重命名或派生表达式。",
        ),
        InterfaceKind::FilterRows => (
            "Filters rows by one source column and a Rust-issued compatible operator.",
            "按一个源列和 Rust 提供的兼容运算符筛选行。",
        ),
        _ => (
            "Uses stable ports and the tabular runtime API.",
            "使用稳定端口和表格运行时 API。",
        ),
    };
    out.extend([
        ("en-US", title, Text(spec.title)),
        ("zh-CN", title, Text(spec.zh_title)),
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
