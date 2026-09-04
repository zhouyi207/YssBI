use super::support::*;
use yss_graph_protocol::*;

#[derive(Clone, Copy)]
struct SeriesConversionSpec {
    source: &'static str,
    target: &'static str,
    title: &'static str,
    zh_title: &'static str,
}

const SERIES_CONVERSIONS: &[SeriesConversionSpec] = &[
    SeriesConversionSpec {
        source: "string",
        target: "categorical",
        title: "String to Categorical",
        zh_title: "字符串转分类",
    },
    SeriesConversionSpec {
        source: "string",
        target: "float64",
        title: "String to Float64",
        zh_title: "字符串转 Float64",
    },
    SeriesConversionSpec {
        source: "string",
        target: "int64",
        title: "String to Int64",
        zh_title: "字符串转 Int64",
    },
    SeriesConversionSpec {
        source: "int64",
        target: "string",
        title: "Int64 to String",
        zh_title: "Int64 转字符串",
    },
    SeriesConversionSpec {
        source: "float64",
        target: "string",
        title: "Float64 to String",
        zh_title: "Float64 转字符串",
    },
    SeriesConversionSpec {
        source: "int64",
        target: "float64",
        title: "Int64 to Float64",
        zh_title: "Int64 转 Float64",
    },
    SeriesConversionSpec {
        source: "float64",
        target: "int64",
        title: "Float64 to Int64",
        zh_title: "Float64 转 Int64",
    },
    SeriesConversionSpec {
        source: "int64",
        target: "bool",
        title: "Int64 to Boolean",
        zh_title: "Int64 转布尔值",
    },
    SeriesConversionSpec {
        source: "float64",
        target: "bool",
        title: "Float64 to Boolean",
        zh_title: "Float64 转布尔值",
    },
    SeriesConversionSpec {
        source: "categorical",
        target: "string",
        title: "Categorical to String",
        zh_title: "分类转字符串",
    },
    SeriesConversionSpec {
        source: "int64",
        target: "categorical",
        title: "Int64 to Categorical",
        zh_title: "Int64 转分类",
    },
    SeriesConversionSpec {
        source: "categorical",
        target: "int64",
        title: "Categorical to Int64",
        zh_title: "分类转 Int64",
    },
    SeriesConversionSpec {
        source: "float64",
        target: "categorical",
        title: "Float64 to Categorical",
        zh_title: "Float64 转分类",
    },
    SeriesConversionSpec {
        source: "categorical",
        target: "float64",
        title: "Categorical to Float64",
        zh_title: "分类转 Float64",
    },
];

pub(super) fn register(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    register_scalar_convert(fragment)?;
    for spec in SERIES_CONVERSIONS {
        register_series_convert(fragment, *spec)?;
    }
    Ok(())
}

fn register_scalar_convert(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    const ID: &str = "yssbi.value.convert";
    fragment.add_node_messages(&NodeTextSpec {
        id: ID,
        title: "Convert Value",
        zh_title: "转换值",
        documentation: "Supported targets are Boolean, Int64, Float64, and String. Invalid or lossy conversions fail explicitly.",
        zh_documentation: "支持布尔值、Int64、Float64 和字符串；无效或有损转换会显式失败。",
        aliases: &["convert", "cast", "coerce", "type conversion"],
        zh_aliases: &["转换", "类型转换", "强制转换"],
    })?;
    add_parameter_messages(
        fragment,
        ID,
        &[(
            "target_type",
            "Target Type",
            "目标类型",
            "Stable core type identifier used by the conversion kernel.",
            "转换 kernel 使用的稳定核心类型标识。",
        )],
    )?;
    let target = parameter(
        ID,
        "target_type",
        concrete("core.string")?,
        None,
        vec![
            ParameterConstraint::Required,
            ParameterConstraint::OneOf(vec![
                Value::String("core.bool".into()),
                Value::String("core.int64".into()),
                Value::String("core.float64".into()),
                Value::String("core.string".into()),
            ]),
        ],
        ParameterEditorSpec::Select,
    )?;
    let scalar_types = scalar_conversion_types()?;
    let mut protocol = protocol(
        ID,
        "conversion",
        vec![
            data_port("input", "Input", PortDirection::Input, scalar_types.clone())?,
            data_port("output", "Output", PortDirection::Output, scalar_types)?,
        ],
        vec![],
        vec![target],
        pure(),
    )?;
    protocol.typing = NodeTypingSpec::ParameterOutput {
        parameter: semantic("target_type", ParameterKey::new)?,
        output: semantic("output", PortKey::new)?,
    };
    fragment.nodes.push(leaf(protocol, ID));
    Ok(())
}

fn register_series_convert(
    fragment: &mut ProviderFragment,
    spec: SeriesConversionSpec,
) -> Result<(), BuiltinAssemblyError> {
    let id = leak(format!(
        "yssbi.data_series.convert.{}_to_{}",
        spec.source, spec.target
    ));
    let source_term = leak(format!("DataSeries<{}>", technical_type(spec.source)?));
    let target_term = leak(format!("DataSeries<{}>", technical_type(spec.target)?));
    let aliases = Box::leak(
        vec![
            "data series conversion",
            "series cast",
            source_term,
            target_term,
        ]
        .into_boxed_slice(),
    );
    fragment.add_node_messages(&NodeTextSpec {
        id,
        title: spec.title,
        zh_title: spec.zh_title,
        documentation: "The kernel preserves element order and nulls. Parse and range failures are reported instead of silently replacing values.",
        zh_documentation: "kernel 保持元素顺序与空值；解析或范围错误会显式报告，不会静默替换。",
        aliases,
        zh_aliases: &["数据序列转换", "序列类型转换"],
    })?;
    fragment.nodes.push(leaf(
        protocol(
            id,
            "conversion",
            vec![
                data_port(
                    "input",
                    "DataSeries",
                    PortDirection::Input,
                    data_series(core_type(spec.source)?)?,
                )?,
                data_port(
                    "output",
                    "DataSeries",
                    PortDirection::Output,
                    data_series(core_type(spec.target)?)?,
                )?,
            ],
            vec![],
            vec![],
            pure(),
        )?,
        id,
    ));
    Ok(())
}

fn scalar_conversion_types() -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(TypeExpr::Union(vec![
        concrete("core.bool")?,
        concrete("core.int64")?,
        concrete("core.float64")?,
        concrete("core.string")?,
    ]))
}

fn core_type(kind: &'static str) -> Result<&'static str, BuiltinAssemblyError> {
    match kind {
        "bool" => Ok("core.bool"),
        "int64" => Ok("core.int64"),
        "float64" => Ok("core.float64"),
        "string" => Ok("core.string"),
        "categorical" => Ok("core.categorical"),
        _ => Err(invalid_conversion_type(kind)),
    }
}

fn technical_type(kind: &'static str) -> Result<&'static str, BuiltinAssemblyError> {
    match kind {
        "bool" => Ok("Boolean"),
        "int64" => Ok("Int64"),
        "float64" => Ok("Float64"),
        "string" => Ok("String"),
        "categorical" => Ok("Categorical"),
        _ => Err(invalid_conversion_type(kind)),
    }
}

fn invalid_conversion_type(value: &'static str) -> BuiltinAssemblyError {
    BuiltinAssemblyError::InvalidProtocol {
        node_type: "yssbi.data_series.convert".into(),
        source: ProtocolError::InvalidIdentity(format!(
            "unsupported series conversion type '{value}'"
        )),
    }
}
