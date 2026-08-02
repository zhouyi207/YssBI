//! DataFrame node protocols staged for aggregation into the built-in provider.
//!
//! This module intentionally depends only on the new node-system IR. It does not
//! use legacy `NodeDefinition`, `PinRole`, `GraphInstance`, or `Executor` types.

mod families;

use super::builtin::{ProviderFragment, iid, leaf, sid};
use super::localization::{Aliases, Message, Text};
use crate::node_system::compiler::{
    FragmentMetadata, FragmentResult, LoweredKernel, LoweredNode, LoweringContext, LoweringError,
    NodeImplementation, NodeLowerer, RelationalInputBinding, RelationalNodeFragment,
};
use crate::node_system::document::PortRef;
use crate::node_system::plan::{
    CompiledParameterHandle, RelationalBackendId, RelationalFragmentId, RelationalOperator,
    RelationalOperatorIndex, RelationalRename, ResourceId,
};
use crate::node_system::protocol::*;
use crate::node_system::registry::{CategoryRegistration, RegisteredNode, TypeRegistration};
use std::sync::Arc;

#[cfg(test)]
pub use families::LEGACY_NODE_IDS;
use families::{InterfaceKind, NODES, NodeSpec};

pub const DATAFRAME_COLUMNS_RESOLVER: &str = "yssbi.dataframe.interface.columns";
pub const DATAFRAME_RESOURCE_SCHEMA_RESOLVER: &str = "yssbi.dataframe.schema.resource";
pub const DATAFRAME_PANEL_SCHEMA_RESOLVER: &str = "yssbi.dataframe.schema.panel";

pub(crate) fn build_provider_fragment() -> ProviderFragment {
    let mut messages = Vec::new();
    add_shared_messages(&mut messages);
    let nodes = NODES
        .iter()
        .map(|spec| {
            add_node_messages(&mut messages, spec);
            registered_node(spec)
        })
        .collect();

    ProviderFragment {
        types: dataframe_types(),
        categories: dataframe_categories(),
        interface_resolvers: vec![sid(DATAFRAME_COLUMNS_RESOLVER, InterfaceResolverId::new)],
        schema_resolvers: vec![
            sid(DATAFRAME_RESOURCE_SCHEMA_RESOLVER, SchemaResolverId::new),
            sid(DATAFRAME_PANEL_SCHEMA_RESOLVER, SchemaResolverId::new),
        ],
        nodes,
        messages,
        ..ProviderFragment::default()
    }
}

fn registered_node(spec: &NodeSpec) -> RegisteredNode {
    let protocol = protocol(spec);
    match spec.interface {
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
        _ => leaf(protocol, spec.kernel),
    }
}

fn protocol(spec: &NodeSpec) -> NodeProtocol {
    let (ports, parameters) = interface(spec.interface);
    NodeProtocol {
        type_id: sid(spec.id, NodeTypeId::new),
        catalog: NodeCatalogProtocol {
            title_key: node_key(spec.id, "title"),
            description_key: Some(node_key(spec.id, "description")),
            documentation_key: Some(node_key(spec.id, "documentation")),
            aliases_key: Some(node_key(spec.id, "aliases")),
            category_id: sid(category(spec.interface), NodeCategoryId::new),
            icon_id: sid("builtin.dataframe", IconId::new),
            style_id: sid("builtin.dataframe", NodeStyleId::new),
            hidden: false,
        },
        interface: NodeInterfaceProtocol::new(ports, vec![], vec![])
            .expect("dataframe node interface"),
        parameters: ParameterSchema::new(parameters).expect("dataframe node parameters"),
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            purity: Purity::Pure,
            evaluation: EvaluationPolicy::DemandDriven,
            cache: CachePolicy::PerRun,
            effects: EffectSemantics::None,
        },
        scope: NodeScope::Any,
        managed_role: None,
    }
}

fn interface(kind: InterfaceKind) -> (Vec<PortSpec>, Vec<ParameterSpec>) {
    use InterfaceKind::*;
    match kind {
        DataframeSource => (
            vec![streaming_output(
                "dataframe",
                dataframe_type(),
                Some(derived_schema(DATAFRAME_RESOURCE_SCHEMA_RESOLVER, vec![])),
            )],
            vec![resource_parameter("dataframe")],
        ),
        Limit => (
            vec![
                streaming_input("source", dataframe_type(), None),
                streaming_output(
                    "result",
                    dataframe_type(),
                    Some(SchemaExpr::Input(port_key("source"))),
                ),
            ],
            vec![bounded_positive_integer_parameter("rows", 100, 1_000_000)],
        ),
        Rename => (
            vec![
                streaming_input("source", dataframe_type(), None),
                streaming_output(
                    "result",
                    dataframe_type(),
                    Some(SchemaExpr::Rename {
                        input: Box::new(SchemaExpr::Input(port_key("source"))),
                        mapping: RenameExpr::FromParameters {
                            from: sid("from", ParameterKey::new),
                            to: sid("to", ParameterKey::new),
                        },
                    }),
                ),
            ],
            vec![
                required_text_parameter("from"),
                required_text_parameter("to"),
            ],
        ),
        Decompose => (
            vec![
                data_input("dataframe", dataframe_type(), None),
                derived_output("columns", series_type(), DATAFRAME_COLUMNS_RESOLVER),
            ],
            vec![],
        ),
        Combine => (
            vec![
                user_input("series", series_type(), 1),
                data_output(
                    "dataframe",
                    dataframe_type(),
                    Some(SchemaExpr::Append {
                        inputs: vec![SchemaExpr::Input(port_key("series"))],
                    }),
                ),
            ],
            vec![],
        ),
        Filter => (
            vec![
                data_input("source", dataframe_type(), None),
                data_input("condition", bool_series_type(), None),
                data_output(
                    "result",
                    dataframe_type(),
                    Some(SchemaExpr::Filter {
                        input: Box::new(SchemaExpr::Input(port_key("source"))),
                    }),
                ),
            ],
            vec![],
        ),
        SeriesSelect => (
            vec![
                data_input("dataframe", dataframe_type(), None),
                data_output("series", series_type(), None),
            ],
            vec![column_parameter("column")],
        ),
        IntRange => (
            vec![
                scalar_input("start", "core.int64"),
                scalar_input("end", "core.int64"),
                scalar_input("step", "core.int64"),
                data_output("series", int_series_type(), None),
            ],
            vec![],
        ),
        SeriesUnaryScalar => (
            vec![
                data_input("series", series_type(), None),
                data_output("value", float_type(), None),
            ],
            vec![],
        ),
        SeriesCompare => (
            vec![
                data_input("left", series_type(), None),
                data_input("right", series_or_scalar_type(), None),
                data_output("result", bool_series_type(), None),
            ],
            vec![],
        ),
        Standardize => (
            vec![
                data_input("series", float_series_type(), None),
                data_output("standardized", float_series_type(), None),
                data_output("mean", float_type(), None),
                data_output("standard_deviation", float_type(), None),
            ],
            vec![],
        ),
        InverseStandardize => (
            vec![
                data_input("standardized", float_series_type(), None),
                scalar_input("mean", "core.float64"),
                scalar_input("standard_deviation", "core.float64"),
                data_output("series", float_series_type(), None),
            ],
            vec![],
        ),
        DummyInfo => (
            vec![
                data_input("source", series_type(), None),
                data_output("result", series_type(), None),
            ],
            vec![text_parameter("base_level", false)],
        ),
        TimeAlign => (
            vec![
                data_input("dataframe", dataframe_type(), None),
                data_input("time", series_type(), None),
                data_output(
                    "aligned",
                    dataframe_type(),
                    Some(SchemaExpr::Input(port_key("dataframe"))),
                ),
            ],
            vec![select_parameter("frequency")],
        ),
        TimeUnary => (
            vec![
                data_input("series", float_series_type(), None),
                data_output("result", float_series_type(), None),
            ],
            vec![positive_integer_parameter("order", 1)],
        ),
        TimeWindow => (
            vec![
                data_input("series", float_series_type(), None),
                data_output("result", float_series_type(), None),
            ],
            vec![positive_integer_parameter("window", 1)],
        ),
        PanelAlign => (
            vec![
                data_input("dataframe", dataframe_type(), None),
                data_input("entity", series_type(), None),
                data_input("time", series_type(), None),
                data_output(
                    "aligned",
                    dataframe_type(),
                    Some(derived_schema(
                        DATAFRAME_PANEL_SCHEMA_RESOLVER,
                        vec![SchemaDependency::Port(port_key("dataframe"))],
                    )),
                ),
            ],
            vec![],
        ),
        PanelDifference => (
            vec![
                data_input("aligned", dataframe_type(), None),
                data_input("series", float_series_type(), None),
                data_output("result", float_series_type(), None),
            ],
            vec![positive_integer_parameter("order", 1)],
        ),
    }
}

fn data_input(key: &'static str, value_type: TypeExpr, schema: Option<SchemaExpr>) -> PortSpec {
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
) -> PortSpec {
    let mut spec = data_input(key, value_type, schema);
    spec.consumption = Some(InputConsumption::Streaming);
    spec
}

fn scalar_input(key: &'static str, type_id: &'static str) -> PortSpec {
    data_input(key, concrete(type_id), None)
}

fn user_input(key: &'static str, value_type: TypeExpr, min: u16) -> PortSpec {
    port(
        key,
        PortDirection::Input,
        value_type,
        PortInstances::UserCreated { min, max: None },
        None,
    )
}

fn data_output(key: &'static str, value_type: TypeExpr, schema: Option<SchemaExpr>) -> PortSpec {
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
) -> PortSpec {
    let mut spec = data_output(key, value_type, schema);
    spec.production = Some(OutputProduction::Streaming);
    spec
}

fn derived_output(key: &'static str, value_type: TypeExpr, resolver: &'static str) -> PortSpec {
    port(
        key,
        PortDirection::Output,
        value_type,
        PortInstances::Derived {
            resolver: sid(resolver, InterfaceResolverId::new),
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
) -> PortSpec {
    PortSpec {
        key: port_key(key),
        label_key: iid(leak(format!("ports.{key}.label"))),
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
    }
}

fn resource_parameter(key: &'static str) -> ParameterSpec {
    parameter(
        key,
        concrete("core.string"),
        ParameterEditorSpec::Resource,
        None,
        vec![ParameterConstraint::Required],
    )
}

fn column_parameter(key: &'static str) -> ParameterSpec {
    parameter(
        key,
        concrete("core.string"),
        ParameterEditorSpec::Select,
        None,
        vec![ParameterConstraint::Required],
    )
}

fn text_parameter(key: &'static str, multiline: bool) -> ParameterSpec {
    parameter(
        key,
        concrete("core.string"),
        ParameterEditorSpec::Text { multiline },
        None,
        vec![],
    )
}

fn required_text_parameter(key: &'static str) -> ParameterSpec {
    parameter(
        key,
        concrete("core.string"),
        ParameterEditorSpec::Text { multiline: false },
        None,
        vec![ParameterConstraint::Required],
    )
}

fn select_parameter(key: &'static str) -> ParameterSpec {
    parameter(
        key,
        concrete("core.string"),
        ParameterEditorSpec::Select,
        None,
        vec![ParameterConstraint::Required],
    )
}

fn positive_integer_parameter(key: &'static str, default: i64) -> ParameterSpec {
    parameter(
        key,
        concrete("core.int64"),
        ParameterEditorSpec::Number,
        Some(ParameterValue {
            value_type: concrete("core.int64"),
            value: Value::Integer(default),
        }),
        vec![ParameterConstraint::IntegerRange {
            min: Some(1),
            max: None,
        }],
    )
}

fn bounded_positive_integer_parameter(key: &'static str, default: i64, max: i64) -> ParameterSpec {
    let mut spec = positive_integer_parameter(key, default);
    spec.constraints = vec![ParameterConstraint::IntegerRange {
        min: Some(1),
        max: Some(max),
    }];
    spec
}

fn parameter(
    key: &'static str,
    value_type: TypeExpr,
    editor: ParameterEditorSpec,
    default_value: Option<ParameterValue>,
    constraints: Vec<ParameterConstraint>,
) -> ParameterSpec {
    ParameterSpec {
        key: sid(key, ParameterKey::new),
        title_key: iid(leak(format!("parameters.{key}.title"))),
        description_key: Some(iid(leak(format!("parameters.{key}.description")))),
        value_type,
        default_value,
        constraints,
        editor,
    }
}

fn dataframe_types() -> Vec<TypeRegistration> {
    [
        ("tabular.dataframe", "types.dataframe.title"),
        ("tabular.series", "types.series.title"),
    ]
    .into_iter()
    .map(|(id, title)| TypeRegistration {
        id: sid(id, TypeId::new),
        title_key: iid(title),
        classes: Default::default(),
    })
    .collect()
}

fn dataframe_categories() -> Vec<CategoryRegistration> {
    [
        ("dataframe", None, 60),
        ("dataframe.series", Some("dataframe"), 61),
        ("dataframe.timeseries", Some("dataframe"), 62),
        ("dataframe.panel", Some("dataframe"), 63),
    ]
    .into_iter()
    .map(|(id, parent, order)| CategoryRegistration {
        id: sid(id, NodeCategoryId::new),
        title_key: iid(leak(format!("categories.{id}.title"))),
        parent: parent.map(|value| sid(value, NodeCategoryId::new)),
        order,
    })
    .collect()
}

fn category(kind: InterfaceKind) -> &'static str {
    match kind {
        InterfaceKind::DataframeSource
        | InterfaceKind::Limit
        | InterfaceKind::Rename
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

struct SourceLowerer;

impl NodeLowerer for SourceLowerer {
    fn lower(&self, context: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        let resource = context
            .parameters
            .get(&ParameterKey::new("dataframe").expect("static parameter key"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                LoweringError::new("source parameter 'dataframe' must be a resource id")
            })?;
        let resource =
            ResourceId::new(resource).map_err(|error| LoweringError::new(error.to_string()))?;
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
            .ok_or_else(|| LoweringError::new("rename input 'source' was not materialized"))?;
        let result = context
            .outputs
            .iter()
            .find(|(address, _)| {
                matches!(&address.port, PortRef::Declared { key } if key.as_str() == "result")
            })
            .map(|(address, _)| address.clone())
            .ok_or_else(|| LoweringError::new("rename output 'result' was not materialized"))?;
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
    context
        .parameters
        .get(&ParameterKey::new(key).expect("static parameter key"))
        .and_then(serde_json::Value::as_str)
        .map(Into::into)
        .ok_or_else(|| LoweringError::new(format!("rename parameter '{key}' must be a string")))
}

struct LimitLowerer;

impl NodeLowerer for LimitLowerer {
    fn lower(&self, context: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        let rows = context
            .parameters
            .get(&ParameterKey::new("rows").expect("static parameter key"))
            .and_then(serde_json::Value::as_u64)
            .filter(|rows| (1..=1_000_000).contains(rows))
            .ok_or_else(|| {
                LoweringError::new("limit parameter 'rows' must be between 1 and 1000000")
            })?;
        let input = context
            .inputs
            .iter()
            .find(|(address, _)| {
                matches!(&address.port, PortRef::Declared { key } if key.as_str() == "source")
            })
            .map(|(address, _)| address.clone())
            .ok_or_else(|| LoweringError::new("limit input 'source' was not materialized"))?;
        let result = context
            .outputs
            .iter()
            .find(|(address, _)| {
                matches!(&address.port, PortRef::Declared { key } if key.as_str() == "result")
            })
            .map(|(address, _)| address.clone())
            .ok_or_else(|| LoweringError::new("limit output 'result' was not materialized"))?;
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
                .map_err(|error| LoweringError::new(error.to_string()))?,
            fragment: crate::node_system::compiler::relational::RelationalFragment {
                id: RelationalFragmentId::new(format!("node.{}", context.node_id))
                    .map_err(|error| LoweringError::new(error.to_string()))?,
                operators: operators.into_boxed_slice(),
                root,
            },
            inputs,
            metadata,
        }),
        parameters: CompiledParameterHandle::new(format!("node.{}", context.node_id))
            .map_err(|error| LoweringError::new(error.to_string()))?,
    })
}

fn dataframe_type() -> TypeExpr {
    concrete("tabular.dataframe")
}
fn series_type() -> TypeExpr {
    concrete("tabular.series")
}
fn float_type() -> TypeExpr {
    concrete("core.float64")
}
fn int_series_type() -> TypeExpr {
    series_type()
}
fn float_series_type() -> TypeExpr {
    series_type()
}
fn bool_series_type() -> TypeExpr {
    series_type()
}
fn series_or_scalar_type() -> TypeExpr {
    TypeExpr::Union(vec![series_type(), float_type()])
}
fn concrete(id: &'static str) -> TypeExpr {
    TypeExpr::Concrete(sid(id, TypeId::new))
}
fn port_key(key: &'static str) -> PortKey {
    sid(key, PortKey::new)
}
fn derived_schema(resolver: &'static str, dependencies: Vec<SchemaDependency>) -> SchemaExpr {
    SchemaExpr::Derived {
        resolver: sid(resolver, SchemaResolverId::new),
        dependencies,
    }
}
fn node_key(id: &'static str, suffix: &'static str) -> I18nKey {
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
    let (en_description, zh_description, en_documentation, zh_documentation) =
        if spec.interface == InterfaceKind::Rename {
            (
                "Renames one DataFrame column.",
                "重命名数据框中的一列。",
                "Renames the column identified by 'from' to 'to'.",
                "将“源列”指定的列重命名为“目标列”。",
            )
        } else {
            (
                "Performs a typed tabular operation.",
                "执行类型化的表格数据操作。",
                "Uses stable ports and the tabular runtime API.",
                "使用稳定端口和表格运行时 API。",
            )
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
