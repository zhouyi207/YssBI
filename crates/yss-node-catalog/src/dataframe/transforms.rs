//! Native-plan data transformations and their authoring contracts.
use super::*;

#[derive(Clone, Copy)]
enum Kind {
    Impute,
    Sort,
    Deduplicate,
    SetColumn,
    Mask,
    Unpivot,
    Pivot,
    Resample,
    Choose,
    IsNull,
    Fill,
    Map,
    Trim,
    Lower,
    Upper,
    Replace,
    Substring,
    Split,
    Concatenate,
    DatePart,
    DateTruncate,
    DateAdd,
    DateDifference,
    Clip,
    Bin,
    Cumulative,
    Rank,
    ForwardFill,
    BackwardFill,
    Encode,
}
struct Entry {
    id: &'static str,
    en: &'static str,
    zh: &'static str,
    kind: Kind,
    aliases: &'static [&'static str],
    en_help: &'static str,
    zh_help: &'static str,
}
macro_rules! entry {
    ($id:literal, $en:literal, $zh:literal, $kind:ident, $help:literal) => {
        Entry {
            id: $id,
            en: $en,
            zh: $zh,
            kind: Kind::$kind,
            aliases: &[$en, $zh],
            en_help: include_str!(concat!("../docs/en/", $help, ".md")),
            zh_help: include_str!(concat!("../docs/zh/", $help, ".md")),
        }
    };
}
const ENTRIES: &[Entry] = &[
    entry!(
        "yssbi.dataframe.impute.single",
        "Single Imputation",
        "单次插补",
        Impute,
        "impute_single"
    ),
    entry!(
        "yssbi.dataframe.sort",
        "Sort Rows",
        "排序行",
        Sort,
        "transform_sort"
    ),
    entry!(
        "yssbi.dataframe.deduplicate",
        "Deduplicate Rows",
        "行去重",
        Deduplicate,
        "transform_deduplicate"
    ),
    entry!(
        "yssbi.dataframe.set_column",
        "Add or Replace Column",
        "添加或替换列",
        SetColumn,
        "transform_set_column"
    ),
    entry!(
        "yssbi.dataframe.filter.mask",
        "Filter by Series",
        "按数列筛选行",
        Mask,
        "transform_filter_mask"
    ),
    entry!(
        "yssbi.dataframe.unpivot",
        "Unpivot",
        "转长表",
        Unpivot,
        "transform_unpivot"
    ),
    entry!(
        "yssbi.dataframe.pivot",
        "Pivot",
        "转宽表",
        Pivot,
        "transform_pivot"
    ),
    entry!(
        "yssbi.dataframe.resample",
        "Resample Time",
        "时间重采样",
        Resample,
        "transform_resample"
    ),
    entry!(
        "yssbi.dataframe.series.choose",
        "Conditional Selection",
        "条件选择",
        Choose,
        "transform_choose"
    ),
    entry!(
        "yssbi.dataframe.series.is_null",
        "Is Missing",
        "缺失判断",
        IsNull,
        "transform_is_null"
    ),
    entry!(
        "yssbi.dataframe.series.is_not_null",
        "Is Present",
        "非缺失判断",
        IsNull,
        "transform_is_not_null"
    ),
    entry!(
        "yssbi.dataframe.series.fill_null",
        "Fill Missing Values",
        "填充缺失值",
        Fill,
        "transform_fill_null"
    ),
    entry!(
        "yssbi.dataframe.series.map",
        "Map Values",
        "值映射",
        Map,
        "transform_map"
    ),
    entry!(
        "yssbi.dataframe.series.text.trim",
        "Trim Text",
        "清理首尾空白",
        Trim,
        "transform_text_trim"
    ),
    entry!(
        "yssbi.dataframe.series.text.lower",
        "Lowercase Text",
        "转小写",
        Lower,
        "transform_text_lower"
    ),
    entry!(
        "yssbi.dataframe.series.text.upper",
        "Uppercase Text",
        "转大写",
        Upper,
        "transform_text_upper"
    ),
    entry!(
        "yssbi.dataframe.series.text.replace",
        "Replace Text",
        "文本替换",
        Replace,
        "transform_text_replace"
    ),
    entry!(
        "yssbi.dataframe.series.text.substring",
        "Substring",
        "文本截取",
        Substring,
        "transform_text_substring"
    ),
    entry!(
        "yssbi.dataframe.series.text.split",
        "Split Text Part",
        "文本拆分取段",
        Split,
        "transform_text_split"
    ),
    entry!(
        "yssbi.dataframe.series.text.concatenate",
        "Concatenate Text",
        "文本拼接",
        Concatenate,
        "transform_text_concatenate"
    ),
    entry!(
        "yssbi.dataframe.series.datetime.part",
        "Extract Date Part",
        "日期字段提取",
        DatePart,
        "transform_datetime_part"
    ),
    entry!(
        "yssbi.dataframe.series.datetime.truncate",
        "Truncate Date",
        "日期截断",
        DateTruncate,
        "transform_datetime_truncate"
    ),
    entry!(
        "yssbi.dataframe.series.datetime.add",
        "Add Date Interval",
        "日期加减",
        DateAdd,
        "transform_datetime_add"
    ),
    entry!(
        "yssbi.dataframe.series.datetime.difference",
        "Date Difference",
        "日期差值",
        DateDifference,
        "transform_datetime_difference"
    ),
    entry!(
        "yssbi.dataframe.series.clip",
        "Clip Values",
        "数值限制",
        Clip,
        "transform_clip"
    ),
    entry!(
        "yssbi.dataframe.series.bin",
        "Bin Values",
        "数值分箱",
        Bin,
        "transform_bin"
    ),
    entry!(
        "yssbi.dataframe.series.cumulative",
        "Cumulative Statistics",
        "累计运算",
        Cumulative,
        "transform_cumulative"
    ),
    entry!(
        "yssbi.dataframe.series.rank",
        "Rank Values",
        "排名",
        Rank,
        "transform_rank"
    ),
    entry!(
        "yssbi.dataframe.series.forward_fill",
        "Forward Fill",
        "前向填充",
        ForwardFill,
        "transform_forward_fill"
    ),
    entry!(
        "yssbi.dataframe.series.backward_fill",
        "Backward Fill",
        "后向填充",
        BackwardFill,
        "transform_backward_fill"
    ),
    entry!(
        "yssbi.dataframe.encode",
        "Generate Indicators",
        "虚拟变量生成",
        Encode,
        "transform_encode"
    ),
];

pub(super) const SCHEMA_RESOLVER: &str = "yssbi.dataframe.schema.transform";
pub(super) fn append(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    fragment
        .schema_resolvers
        .push(sid(SCHEMA_RESOLVER, SchemaResolverId::new)?);
    for entry in ENTRIES {
        let (ports, mut parameters, generic) = interface(entry.kind)?;
        for parameter in &mut parameters {
            let key = parameter.key.as_str();
            let (en, zh, en_description, zh_description) = parameter_text(key);
            parameter.title_key = node_key(entry.id, &format!("parameters.{key}.title"))?;
            parameter.description_key = Some(node_key(
                entry.id,
                &format!("parameters.{key}.description"),
            )?);
            for (locale, title, description) in
                [("en-US", en, en_description), ("zh-CN", zh, zh_description)]
            {
                fragment.messages.push((
                    locale,
                    parameter.title_key.as_str().to_owned(),
                    Text(title),
                ));
                fragment.messages.push((
                    locale,
                    parameter
                        .description_key
                        .as_ref()
                        .unwrap()
                        .as_str()
                        .to_owned(),
                    Text(description),
                ));
            }
        }
        for (locale, title) in [("en-US", entry.en), ("zh-CN", entry.zh)] {
            fragment
                .messages
                .push((locale, node_key_text(entry.id, "title"), Text(title)));
            fragment.messages.push((
                locale,
                node_key_text(entry.id, "aliases"),
                Aliases(entry.aliases),
            ));
            fragment.messages.push((
                locale,
                node_key_text(entry.id, "documentation"),
                Text(title),
            ));
        }
        let frame = matches!(
            entry.kind,
            Kind::Sort
                | Kind::Deduplicate
                | Kind::SetColumn
                | Kind::Mask
                | Kind::Unpivot
                | Kind::Pivot
                | Kind::Resample
        );
        let protocol = NodeProtocol {
            type_id: sid(entry.id, NodeTypeId::new)?,
            catalog: NodeCatalogProtocol {
                title_key: node_key(entry.id, "title")?,
                documentation_key: Some(node_key(entry.id, "documentation")?),
                aliases_key: Some(node_key(entry.id, "aliases")?),
                category_id: sid(
                    if matches!(entry.kind, Kind::Impute) {
                        "statistics.imputation"
                    } else if frame {
                        "dataframe"
                    } else {
                        "dataframe.series"
                    },
                    NodeCategoryId::new,
                )?,
                icon_id: sid("builtin.dataframe", IconId::new)?,
                style_id: sid("builtin.dataframe", NodeStyleId::new)?,
                hidden: false,
            },
            interface: assembled_interface(
                entry.id,
                ports,
                if generic {
                    vec![sid("element", TypeParameterId::new)?]
                } else {
                    vec![]
                },
                vec![],
            )?,
            parameters: assembled_parameters(entry.id, parameters)?,
            instance_display: NodeInstanceDisplaySpec::Static,
            execution: ExecutionSemantics {
                determinism: Determinism::Deterministic,
                cache: CachePolicy::PerRun,
            },
            typing: NodeTypingSpec::Fixed,
            scope: NodeScope::Any,
            managed_role: None,
        };
        fragment.nodes.push(leaf(protocol, entry.id));
    }
    Ok(())
}

fn list(key: &'static str, element: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    let value_type = data_series_type(concrete(element)?);
    parameter(
        key,
        value_type.clone(),
        ParameterEditorSpec::Auto,
        Some(TypedValue {
            value_type,
            value: DataValue::List(vec![]),
        }),
        vec![],
    )
}
fn columns(key: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    let mut parameter = list(key, "core.text")?;
    parameter.constraints.push(ParameterConstraint::ColumnNames);
    Ok(parameter)
}

fn toggle(key: &'static str, value: bool) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.binary")?,
        ParameterEditorSpec::Toggle,
        Some(TypedValue {
            value_type: concrete("core.binary")?,
            value: DataValue::Bool(value),
        }),
        vec![],
    )
}
fn text_value(key: &'static str, value: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.text")?,
        ParameterEditorSpec::Text { multiline: false },
        Some(TypedValue {
            value_type: concrete("core.text")?,
            value: DataValue::String(value.into()),
        }),
        vec![],
    )
}
fn number(
    key: &'static str,
    value: i64,
    min: Option<i64>,
) -> Result<Parameter, BuiltinAssemblyError> {
    parameter(
        key,
        concrete("core.numeric")?,
        ParameterEditorSpec::Number,
        Some(TypedValue {
            value_type: concrete("core.numeric")?,
            value: DataValue::Integer(value),
        }),
        vec![ParameterConstraint::IntegerRange { min, max: None }],
    )
}
pub(super) fn offset_parameter() -> Result<Parameter, BuiltinAssemblyError> {
    number("offset", 0, Some(0))
}
pub(super) fn minimum_periods_parameter() -> Result<Parameter, BuiltinAssemblyError> {
    number("min_periods", 0, Some(0))
}
pub(crate) fn documentation(id: &str, locale: &str) -> Option<Box<str>> {
    let entry = ENTRIES.iter().find(|entry| entry.id == id)?;
    Some(
        if crate::documentation::is_chinese_locale(locale) {
            entry.zh_help
        } else {
            entry.en_help
        }
        .into(),
    )
}
pub(super) fn window_context() -> Result<PortSpec, BuiltinAssemblyError> {
    let mut context = user_input("context", "Context", dataframe_type()?, 0)?;
    context.cardinality = PortCardinality::UserCreated {
        min: 0,
        max: Some(1),
    };
    context.consumption = Some(InputConsumption::Streaming);
    Ok(context)
}
pub(super) fn window_parameters() -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    Ok(vec![
        columns("partition_by")?,
        columns("order_by")?,
        toggle("descending", false)?,
        toggle("nulls_first", false)?,
    ])
}
fn mixed(element: TypeExpr) -> TypeExpr {
    let mut members = if matches!(element, TypeExpr::Generic(_)) {
        yss_data_contract::SemanticType::ALL
            .into_iter()
            .map(|semantic| {
                TypeExpr::Concrete(TypeId::new(semantic.type_id()).expect("semantic type id"))
            })
            .collect::<Vec<_>>()
    } else {
        vec![element.clone()]
    };
    members.push(data_series_type(element));
    TypeExpr::Union(members)
}
fn interface(kind: Kind) -> Result<(Vec<PortSpec>, Vec<Parameter>, bool), BuiltinAssemblyError> {
    use Kind::*;
    let generic = matches!(
        kind,
        SetColumn | Choose | IsNull | Fill | Map | ForwardFill | BackwardFill
    );
    let element = TypeExpr::Generic(sid("element", TypeParameterId::new)?);
    let generic_series = data_series_type(element.clone());
    let frame = matches!(
        kind,
        Sort | Deduplicate | SetColumn | Mask | Unpivot | Pivot | Resample
    );
    let mut parameters = Vec::new();
    let mut ports = if frame {
        relational_ports(derived_schema(
            SCHEMA_RESOLVER,
            vec![SchemaDependency::Port(port_key("source")?)],
        )?)?
    } else {
        let (input, output) = match kind {
            Choose => (
                data_series_type(concrete("core.binary")?),
                generic_series.clone(),
            ),
            IsNull => (
                generic_series.clone(),
                data_series_type(concrete("core.binary")?),
            ),
            Fill | Map | ForwardFill | BackwardFill => {
                (generic_series.clone(), generic_series.clone())
            }
            Trim | Lower | Upper | Replace | Substring | Split | Concatenate => (
                data_series_type(concrete("core.text")?),
                data_series_type(concrete("core.text")?),
            ),
            DatePart | DateDifference => (
                data_series_type(concrete("core.datetime")?),
                numeric_series_type(),
            ),
            DateTruncate | DateAdd => (
                data_series_type(concrete("core.datetime")?),
                data_series_type(concrete("core.datetime")?),
            ),
            Bin => (
                numeric_series_type(),
                data_series_type(concrete("core.ordinal")?),
            ),
            Rank => (
                TypeExpr::Union(
                    ["core.numeric", "core.text", "core.datetime", "core.ordinal"]
                        .into_iter()
                        .map(|id| concrete(id).map(data_series_type))
                        .collect::<Result<_, _>>()?,
                ),
                numeric_series_type(),
            ),
            Encode => (
                TypeExpr::Union(
                    ["core.categorical", "core.ordinal", "core.binary"]
                        .into_iter()
                        .map(|id| concrete(id).map(data_series_type))
                        .collect::<Result<_, _>>()?,
                ),
                dataframe_type()?,
            ),
            _ => (numeric_series_type(), numeric_series_type()),
        };
        vec![
            streaming_input(
                if matches!(kind, Choose) {
                    "condition"
                } else {
                    "series"
                },
                "Series",
                input,
                None,
            )?,
            streaming_output(
                "result",
                "Result",
                output,
                if matches!(kind, Encode) {
                    Some(derived_schema(
                        SCHEMA_RESOLVER,
                        vec![SchemaDependency::Port(port_key("series")?)],
                    )?)
                } else {
                    None
                },
            )?,
        ]
    };
    match kind {
        Impute => {
            parameters = vec![
                choice_parameter(
                    "imputation_method",
                    "mean",
                    &["mean", "median", "mode", "constant"],
                )?,
                parameter(
                    "fill_value",
                    concrete("core.numeric")?,
                    ParameterEditorSpec::Number,
                    Some(TypedValue {
                        value_type: concrete("core.numeric")?,
                        value: DataValue::Integer(0),
                    }),
                    vec![],
                )?,
            ];
        }
        Sort => {
            parameters = vec![
                nominal_parameter(
                    "columns",
                    yss_node_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
                )?,
                columns("descending_columns")?,
                toggle("nulls_first", false)?,
            ];
        }
        Deduplicate => {
            parameters = vec![
                columns("keys")?,
                choice_parameter("keep", "first", &["first", "last", "none"])?,
            ];
        }
        SetColumn => {
            ports.insert(
                1,
                streaming_input("series", "Series", generic_series, None)?,
            );
            parameters = vec![text_value("name", "computed")?];
        }
        Mask => {
            ports.insert(
                1,
                streaming_input(
                    "mask",
                    "Mask",
                    data_series_type(concrete("core.binary")?),
                    None,
                )?,
            );
            parameters = vec![toggle("drop_matches", false)?];
        }
        Unpivot => {
            parameters = vec![
                columns("keys")?,
                nominal_parameter(
                    "columns",
                    yss_node_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
                )?,
                text_value("variable_name", "variable")?,
                text_value("value_name", "value")?,
                toggle("include_null", true)?,
            ];
        }
        Pivot => {
            parameters = vec![
                columns("keys")?,
                column_parameter("category_column")?,
                column_parameter("value_column")?,
                list("levels", "core.text")?,
                columns("names")?,
                choice_parameter("aggregate", "sum", &["sum", "mean", "min", "max", "count"])?,
            ];
        }
        Resample => {
            parameters = vec![
                column_parameter("time_column")?,
                choice_parameter(
                    "unit",
                    "day",
                    &[
                        "year", "quarter", "month", "week", "day", "hour", "minute", "second",
                    ],
                )?,
                columns("keys")?,
                nominal_parameter(
                    "columns",
                    yss_node_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
                )?,
                choice_parameter("aggregate", "mean", &["sum", "mean", "min", "max", "count"])?,
            ];
        }
        Choose => {
            ports.insert(
                1,
                streaming_input("when_true", "When True", mixed(element.clone()), None)?,
            );
            ports.insert(
                2,
                streaming_input("when_false", "When False", mixed(element), None)?,
            );
        }
        Fill => {
            ports.insert(
                1,
                streaming_input("replacement", "Replacement", mixed(element), None)?,
            );
        }
        Map => {
            parameters = vec![
                list("from_values", "core.text")?,
                list("to_values", "core.text")?,
                toggle("keep_unmatched", true)?,
            ];
        }
        Replace => {
            parameters = vec![required_text_parameter("from")?, text_value("to", "")?];
        }
        Substring => {
            parameters = vec![number("start", 1, Some(1))?, number("length", 10, Some(0))?];
        }
        Split => {
            parameters = vec![text_value("separator", ",")?, number("part", 1, None)?];
        }
        Concatenate => {
            ports[0] = user_input("parts", "Text", mixed(concrete("core.text")?), 2)?;
            ports[0].consumption = Some(InputConsumption::Streaming);
            parameters = vec![text_value("separator", "")?];
        }
        DatePart => {
            parameters = vec![choice_parameter(
                "part",
                "year",
                &[
                    "year", "quarter", "month", "week", "day", "doy", "dow", "hour", "minute",
                    "second",
                ],
            )?];
        }
        DateTruncate => {
            parameters = vec![choice_parameter(
                "unit",
                "day",
                &[
                    "year", "quarter", "month", "week", "day", "hour", "minute", "second",
                ],
            )?];
        }
        DateAdd => {
            parameters = vec![
                choice_parameter(
                    "unit",
                    "day",
                    &[
                        "year",
                        "month",
                        "week",
                        "day",
                        "hour",
                        "minute",
                        "second",
                        "millisecond",
                        "microsecond",
                        "nanosecond",
                    ],
                )?,
                number("amount", 1, None)?,
            ];
        }
        DateDifference => {
            ports.insert(
                1,
                streaming_input("other", "Other", mixed(concrete("core.datetime")?), None)?,
            );
            parameters = vec![choice_parameter(
                "unit",
                "day",
                &[
                    "week",
                    "day",
                    "hour",
                    "minute",
                    "second",
                    "millisecond",
                    "microsecond",
                    "nanosecond",
                ],
            )?];
        }
        Clip => {
            parameters = vec![number("lower", 0, None)?, number("upper", 1, None)?];
            for parameter in &mut parameters {
                parameter.constraints = vec![];
            }
        }
        Bin => {
            parameters = vec![list("edges", "core.numeric")?, list("labels", "core.text")?];
        }
        Cumulative | Rank | ForwardFill | BackwardFill => {
            ports.insert(1, window_context()?);
            parameters = window_parameters()?;
            if matches!(kind, Cumulative) {
                parameters.insert(
                    0,
                    choice_parameter("operation", "sum", &["sum", "mean", "min", "max", "std"])?,
                );
            }
            if matches!(kind, Rank) {
                parameters.insert(0, toggle("dense", false)?);
            }
        }
        Encode => {
            parameters = vec![
                list("levels", "core.text")?,
                columns("names")?,
                toggle("drop_reference", false)?,
                text_value("reference", "")?,
            ];
        }
        _ => {}
    }
    Ok((ports, parameters, generic))
}

pub(super) fn shared_window_messages(out: &mut Vec<(&'static str, String, Message)>) {
    for key in [
        "partition_by",
        "order_by",
        "descending",
        "nulls_first",
        "operation",
        "min_periods",
        "direction",
        "offset",
    ] {
        let (en, zh, en_description, zh_description) = parameter_text(key);
        for (locale, title, description) in
            [("en-US", en, en_description), ("zh-CN", zh, zh_description)]
        {
            out.push((locale, format!("parameters.{key}.title"), Text(title)));
            out.push((
                locale,
                format!("parameters.{key}.description"),
                Text(description),
            ));
        }
    }
}
fn parameter_text(key: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    match key {
        "imputation_method" => (
            "Imputation method",
            "插补方法",
            "Mean, exact median, numeric mode (smallest tie), or constant.",
            "均值、精确中位数、数值众数（并列取最小值）或常数。",
        ),
        "fill_value" => (
            "Fill value",
            "填补常数",
            "Used only for constant imputation.",
            "仅在常数插补时使用。",
        ),
        "columns" => (
            "Columns",
            "列",
            "Selected columns in output order.",
            "按顺序选择列。",
        ),
        "descending_columns" => (
            "Descending columns",
            "降序列",
            "A subset of the sorting columns.",
            "从排序列中选择需要降序的列。",
        ),
        "keys" => (
            "Keys",
            "键列",
            "Empty selects all columns for deduplication, and no keys for reshaping.",
            "去重时留空检查全部列；宽长表转换时留空表示没有保留键。",
        ),
        "keep" => (
            "Keep",
            "保留方式",
            "First, last, or only rows with unique keys.",
            "保留第一条、最后一条，或仅保留没有重复键的行。",
        ),
        "name" => (
            "Column name",
            "列名",
            "An existing name replaces its column; a new name appends a column.",
            "已有列名替换该列；新列名追加一列。",
        ),
        "drop_matches" => (
            "Remove matching rows",
            "移除匹配行",
            "Removal retains false and null conditions.",
            "移除模式保留条件为假或 Null 的行。",
        ),
        "variable_name" => (
            "Variable column",
            "变量名列",
            "Name of the output variable-name column.",
            "输出中存放原列名的列名。",
        ),
        "value_name" => (
            "Value column",
            "数值列",
            "Name of the output value column.",
            "输出中存放原单元格值的列名。",
        ),
        "include_null" => (
            "Include missing values",
            "包含缺失值",
            "Retain rows whose unpivoted value is Null.",
            "保留转换后取值为 Null 的行。",
        ),
        "category_column" => (
            "Category column",
            "类别列",
            "Values matched by the explicit levels.",
            "与显式类别列表匹配的源列。",
        ),
        "value_column" => (
            "Value column",
            "取值列",
            "Source column to aggregate.",
            "需要聚合的源列。",
        ),
        "levels" => (
            "Category values",
            "类别值",
            "Exact value text; one entry for each output column.",
            "填写原始编码的精确文本，每项对应一个输出列。",
        ),
        "names" => (
            "Output names",
            "输出列名",
            "Matches category values in the same order.",
            "与类别值按相同顺序逐项对应。",
        ),
        "aggregate" => (
            "Aggregation",
            "聚合方式",
            "Numeric aggregations ignore Null; count counts non-null values.",
            "数值聚合忽略 Null；计数只统计非空值。",
        ),
        "from_values" => (
            "Source values",
            "原值",
            "Exact value text, matched in order with replacement values.",
            "原值的精确文本，与替换值逐项对应。",
        ),
        "to_values" => (
            "Replacement values",
            "替换值",
            "Values must fit the source physical type.",
            "替换值必须适合源列的物理类型。",
        ),
        "keep_unmatched" => (
            "Keep unmatched values",
            "保留未匹配值",
            "Otherwise unmatched values become Null.",
            "关闭时未匹配值变为 Null。",
        ),
        "from" => (
            "Find",
            "查找文本",
            "Literal substring to replace.",
            "需要替换的字面文本。",
        ),
        "to" => (
            "Replace with",
            "替换为",
            "Empty text removes matching substrings.",
            "留空可删除匹配文本。",
        ),
        "start" => (
            "Start",
            "起始位置",
            "One-based Unicode character position.",
            "从 1 开始的 Unicode 字符位置。",
        ),
        "length" => (
            "Length",
            "长度",
            "Nonnegative number of Unicode characters.",
            "非负的 Unicode 字符数。",
        ),
        "separator" => (
            "Separator",
            "分隔符",
            "Literal separator for text parts.",
            "文本片段之间的字面分隔符。",
        ),
        "part" => (
            "Part",
            "字段或片段",
            "Date component, or one-based text part; negative parts count from the end.",
            "日期字段，或从 1 开始的文本片段；负数从末尾计数。",
        ),
        "unit" => (
            "Unit",
            "单位",
            "Calendar or duration unit.",
            "日历或时长单位。",
        ),
        "amount" => (
            "Amount",
            "增减量",
            "Negative values subtract the interval.",
            "负数表示减去相应间隔。",
        ),
        "lower" => (
            "Lower bound",
            "下限",
            "Inclusive numeric lower bound.",
            "数值下限。",
        ),
        "upper" => (
            "Upper bound",
            "上限",
            "Must be at least the lower bound.",
            "必须不小于下限。",
        ),
        "edges" => (
            "Bin boundaries",
            "分箱边界",
            "Finite, strictly increasing boundaries.",
            "有限且严格递增的边界。",
        ),
        "labels" => (
            "Bin labels",
            "区间标签",
            "One more label than boundaries, in interval order.",
            "标签数比边界数多一项，按区间顺序填写。",
        ),
        "partition_by" => (
            "Partition columns",
            "分组列",
            "Connect context to choose grouping columns; empty means one group.",
            "连接上下文后选择分组列；留空表示同一组。",
        ),
        "order_by" => (
            "Order columns",
            "排序列",
            "Empty uses the current row order; ties retain source order.",
            "留空使用当前行顺序；同值行保留原顺序。",
        ),
        "descending" => (
            "Descending",
            "降序",
            "Apply descending order to the selected window order columns.",
            "窗口排序列采用降序。",
        ),
        "nulls_first" => (
            "Nulls first",
            "空值靠前",
            "Otherwise Null values sort last.",
            "关闭时空值排在最后。",
        ),
        "operation" => (
            "Operation",
            "运算",
            "Standard deviation uses ddof=1.",
            "标准差采用样本标准差，ddof=1。",
        ),
        "min_periods" => (
            "Minimum valid values",
            "最少有效值",
            "Zero requires a complete non-null window.",
            "零表示要求完整非空窗口。",
        ),
        "direction" => (
            "Direction",
            "移动方向",
            "Lag reads earlier rows; lead reads later rows.",
            "滞后读取前面的行；超前读取后面的行。",
        ),
        "offset" => (
            "Offset",
            "跳过行数",
            "Skip this many rows before taking the requested rows.",
            "取行前先跳过指定数量的行。",
        ),
        "dense" => (
            "Dense ranking",
            "密集排名",
            "Dense ranking leaves no gaps after ties.",
            "密集排名在并列值后不跳号。",
        ),
        "drop_reference" => (
            "Drop reference indicator",
            "省略参照组列",
            "Omit the indicator named by reference.",
            "省略参照组列名所指定的指示列。",
        ),
        "reference" => (
            "Reference column",
            "参照组列名",
            "Must match one explicit output name.",
            "必须与一个显式输出列名相同。",
        ),
        _ => (
            "Time column",
            "时间列",
            "Time column used for grouping.",
            "用于时间分组的源列。",
        ),
    }
}
