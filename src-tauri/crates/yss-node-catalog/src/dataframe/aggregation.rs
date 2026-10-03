use super::*;
use yss_data_contract::aggregation::AggregateOperation;

pub(super) fn interface(
    kind: InterfaceKind,
) -> Result<(Vec<PortSpec>, Vec<Parameter>), BuiltinAssemblyError> {
    let series = kind == InterfaceKind::Frequency;
    let input = if series { "series" } else { "source" };
    let value_type = if series {
        TypeExpr::Union(
            [
                "core.numeric",
                "core.categorical",
                "core.ordinal",
                "core.binary",
            ]
            .into_iter()
            .map(|id| concrete(id).map(data_series_type))
            .collect::<Result<Vec<_>, _>>()?,
        )
    } else {
        dataframe_type()?
    };
    let parameters = match kind {
        InterfaceKind::Frequency => vec![parameter(
            "include_null",
            concrete("core.binary")?,
            ParameterEditorSpec::Toggle,
            Some(TypedValue {
                value_type: concrete("core.binary")?,
                value: DataValue::Bool(true),
            }),
            vec![],
        )?],
        InterfaceKind::GroupBy => {
            let mut parameters = vec![nominal_parameter(
                "keys",
                yss_node_protocol::dataframe::PROJECT_COLUMNS_TYPE_ID,
            )?];
            for operation in AggregateOperation::ALL {
                parameters.push(columns(operation.key())?);
            }
            parameters
        }
        _ => vec![],
    };
    Ok((
        vec![
            streaming_input(
                input,
                if series { "DataSeries" } else { "Source" },
                value_type,
                None,
            )?,
            streaming_output(
                "result",
                "Result",
                dataframe_type()?,
                Some(derived_schema(
                    "yssbi.dataframe.schema.aggregate",
                    vec![SchemaDependency::Port(port_key(input)?)],
                )?),
            )?,
        ],
        parameters,
    ))
}

fn columns(key: &'static str) -> Result<Parameter, BuiltinAssemblyError> {
    let value_type = data_series_type(concrete("core.text")?);
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

pub(super) fn help(kind: InterfaceKind) -> (&'static str, &'static str) {
    match kind {
        InterfaceKind::Frequency => (
            "Counts distinct values of Numeric, Categorical, Ordinal or Binary series. Returns value, frequency and proportion as a complete pageable DataFrame. Include null controls both the missing-value group and the denominator. Ordinal values follow declared levels; other values sort ascending, null last. Empty strings are values.",
            "统计 Numeric、Categorical、Ordinal、Binary 序列的不同取值，输出可分页的完整数据帧：value、frequency、proportion。包含空值同时控制 Null 分组与比例分母；Ordinal 按已声明等级排序，其余按原值升序，Null 在最后。空字符串是有效值。",
        ),
        InterfaceKind::GroupBy => (
            "Groups by one or more keys, including null keys. Always outputs row_count. Choose columns independently for count, sum, mean, min, max, sample standard deviation and median; names are column_operation. Count excludes null; numeric aggregates require Numeric semantics and ignore null. All-null numeric aggregates return null. Duplicate output names are rejected. Output is a pageable DataFrame sorted by keys.",
            "按一个或多个键分组，保留 Null 键分组。始终输出 row_count；分别选择非空计数、求和、均值、最小值、最大值、样本标准差、中位数的列，结果命名为“列名_操作”。数值聚合只接受 Numeric 并忽略 Null；全空数值组返回 Null。拒绝重复输出列名，结果按分组键排序并可分页。",
        ),
        _ => unreachable!("aggregation help requires an aggregation interface"),
    }
}

pub(super) fn messages(out: &mut Vec<(&'static str, String, Message)>) {
    for (key, en, zh, en_help, zh_help) in [
        (
            "include_null",
            "Include Null",
            "包含空值",
            "Include null in frequencies and the denominator.",
            "将 Null 纳入频数表和比例分母。",
        ),
        (
            "keys",
            "Group Keys",
            "分组键",
            "Select one or more grouping columns.",
            "选择一个或多个分组列。",
        ),
        (
            "count",
            "Count Columns",
            "非空计数列",
            "Count non-null values in each selected column.",
            "对所选列分别统计非空值数量。",
        ),
        (
            "sum",
            "Sum Columns",
            "求和列",
            "Numeric columns to sum.",
            "选择需要求和的数值列。",
        ),
        (
            "mean",
            "Mean Columns",
            "均值列",
            "Numeric columns to average.",
            "选择需要计算均值的数值列。",
        ),
        (
            "min",
            "Minimum Columns",
            "最小值列",
            "Numeric columns for minimum.",
            "选择需要计算最小值的数值列。",
        ),
        (
            "max",
            "Maximum Columns",
            "最大值列",
            "Numeric columns for maximum.",
            "选择需要计算最大值的数值列。",
        ),
        (
            "std",
            "Standard Deviation Columns",
            "标准差列",
            "Sample standard deviation, ddof=1.",
            "计算样本标准差，ddof=1。",
        ),
        (
            "median",
            "Median Columns",
            "中位数列",
            "Numeric columns for median.",
            "选择需要计算中位数的数值列。",
        ),
    ] {
        for (locale, title, description) in [("en-US", en, en_help), ("zh-CN", zh, zh_help)] {
            out.push((locale, format!("parameters.{key}.title"), Text(title)));
            out.push((
                locale,
                format!("parameters.{key}.description"),
                Text(description),
            ));
        }
    }
}
