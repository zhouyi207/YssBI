use super::support::*;
use yss_data_contract::DataValue;
use yss_node_protocol::*;

pub(super) fn register(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    let input_types = TypeExpr::Union(
        SemanticType::ALL
            .into_iter()
            .map(|kind| Ok([concrete(kind.type_id())?, data_series(kind.type_id())?]))
            .collect::<Result<Vec<_>, BuiltinAssemblyError>>()?
            .into_iter()
            .flatten()
            .collect(),
    );
    for (id, target, title, zh_title, documentation, zh_documentation) in [
        (
            "yssbi.value.to_numeric",
            SemanticType::Numeric,
            "To Numeric",
            "转数值",
            "Convert a scalar or series to Numeric with exact integer/real conversion and null preservation.",
            "将标量或数列转换为数值，支持精确的整数和实数转换并保留空值。",
        ),
        (
            "yssbi.value.to_text",
            SemanticType::Text,
            "To Text",
            "转文本",
            "Convert a scalar or series to Text using stored values, preserving nulls.",
            "将标量或数列的原始值转换为文本并保留空值。",
        ),
        (
            "yssbi.value.to_categorical",
            SemanticType::Categorical,
            "To Categorical",
            "转分类",
            "Declare or inherit categorical values and labels while preserving original codes and shape.",
            "声明或继承分类取值与标签，保留原始编码及标量或数列结构。",
        ),
        (
            "yssbi.value.to_ordinal",
            SemanticType::Ordinal,
            "To Ordinal",
            "转顺序",
            "Declare or inherit ordered levels while preserving original codes and shape.",
            "声明或继承由低到高的等级顺序，保留原始编码及标量或数列结构。",
        ),
        (
            "yssbi.value.to_binary",
            SemanticType::Binary,
            "To Binary",
            "转二元",
            "Convert a scalar or series to Binary with an optional two-value mapping and positive value.",
            "将标量或数列转换为二元值，可配置两值映射与正值。",
        ),
        (
            "yssbi.value.to_datetime",
            SemanticType::Datetime,
            "To Datetime",
            "转日期时间",
            "Parse or convert calendar values with explicit kind, precision and format while retaining wall time.",
            "按日期时间形式、精度和格式解析或转换日历值，保留原钟面时间。",
        ),
        (
            "yssbi.value.to_identifier",
            SemanticType::Identifier,
            "To Identifier",
            "转标识",
            "Mark a scalar or series as Identifier, preserving original representation without requiring uniqueness.",
            "将标量或数列转换为标识，保留原始表示，不附加唯一性要求。",
        ),
    ] {
        fragment.add_node_messages(&NodeTextSpec {
            id,
            title,
            zh_title,
            documentation,
            zh_documentation,
            aliases: &["convert", "cast", "semantic conversion"],
            zh_aliases: &["转换", "类型转换", "语义转换"],
        })?;
        let parameters = conversion_parameters(fragment, id, target)?;
        let mut input = data_port("input", "Input", PortDirection::Input, input_types.clone())?;
        input.consumption = Some(InputConsumption::Streaming);
        let mut output = data_port(
            "output",
            "Output",
            PortDirection::Output,
            TypeExpr::Union(vec![
                concrete(target.type_id())?,
                data_series(target.type_id())?,
            ]),
        )?;
        output.production = Some(OutputProduction::Streaming);
        let mut protocol = protocol(
            id,
            "conversion",
            vec![input, output],
            vec![],
            parameters,
            pure(),
        )?;
        protocol.typing = NodeTypingSpec::ShapePreservingConversion {
            input: semantic("input", PortKey::new)?,
            target: ConversionTarget::Fixed(target),
            output: semantic("output", PortKey::new)?,
        };
        fragment.nodes.push(leaf(protocol, id));
    }
    Ok(())
}

fn conversion_parameters(
    fragment: &mut ProviderFragment,
    id: &str,
    target: SemanticType,
) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    let mut parameters = Vec::new();
    for (kind, key, default, options, title, zh_title, help, zh_help) in [
        (
            SemanticType::Numeric,
            "numeric_mode",
            "auto",
            &["auto", "integer", "real"][..],
            "Numeric Representation",
            "数值表示",
            "Auto preserves numbers, parses text as real numbers and maps Binary to 0/1. Integer rejects fractions; real requires exact conversion.",
            "自动保留数值表示，文本解析为实数，二元转为 0/1；整数不允许小数；实数转换不得丢失精度。",
        ),
        (
            SemanticType::Datetime,
            "datetime_kind",
            "auto",
            &["auto", "date", "time", "datetime"][..],
            "Calendar Kind",
            "日期时间形式",
            "Auto preserves temporal inputs; text defaults to a date and time.",
            "自动保留已有时间表示，文本默认解析为日期时间；也可明确选择日期或时间。",
        ),
        (
            SemanticType::Datetime,
            "datetime_precision",
            "microseconds",
            &["seconds", "milliseconds", "microseconds", "nanoseconds"][..],
            "Calendar Precision",
            "时间精度",
            "Precision for newly parsed values. Precision loss fails.",
            "新解析时间值使用的精度；不能静默截断更高精度。",
        ),
    ] {
        if kind != target {
            continue;
        }
        add_parameter_messages(fragment, id, &[(key, title, zh_title, help, zh_help)])?;
        parameters.push(parameter(
            id,
            key,
            concrete("core.text")?,
            Some(TypedValue {
                value_type: concrete("core.text")?,
                value: DataValue::String(default.into()),
            }),
            vec![ParameterConstraint::OneOf(
                options
                    .iter()
                    .map(|value| DataValue::String((*value).into()))
                    .collect(),
            )],
            ParameterEditorSpec::Select,
        )?);
    }
    if target == SemanticType::Datetime {
        add_parameter_messages(
            fragment,
            id,
            &[(
                "datetime_format",
                "Calendar Format",
                "时间格式",
                "Empty uses ISO. Custom formats use strftime, e.g. %d/%m/%Y. Offsets are removed without shifting wall time.",
                "留空按 ISO 格式解析。自定义格式如 %d/%m/%Y；移除时区时保留原钟面时间。",
            )],
        )?;
        parameters.push(parameter(
            id,
            "datetime_format",
            concrete("core.text")?,
            Some(TypedValue {
                value_type: concrete("core.text")?,
                value: DataValue::String("".into()),
            }),
            vec![ParameterConstraint::Length {
                min: None,
                max: Some(256),
            }],
            ParameterEditorSpec::Text { multiline: false },
        )?);
    }
    let domain = match target {
        SemanticType::Categorical => Some((
            "Values and Labels",
            "取值与标签",
            "Declare original codes and labels. Empty configuration inherits a compatible source domain.",
            "配置原始编码与标签；留空时继承兼容的源值域。",
        )),
        SemanticType::Ordinal => Some((
            "Levels and Labels",
            "等级与标签",
            "Order original codes and labels from low to high. Empty configuration inherits an existing ordinal domain.",
            "按由低到高的等级配置原始编码与标签；留空时继承已有顺序值域。",
        )),
        SemanticType::Binary => Some((
            "Value Mapping",
            "取值映射",
            "Optionally declare two distinct codes and their positive value. Empty configuration uses supported boolean encodings or the source Binary domain.",
            "可配置两个不同编码及正值；留空时使用支持的布尔编码或源二元值域。",
        )),
        _ => None,
    };
    if let Some((title, zh_title, help, zh_help)) = domain {
        add_parameter_messages(
            fragment,
            id,
            &[("semantic_domain", title, zh_title, help, zh_help)],
        )?;
        parameters.push(parameter(
            id,
            "semantic_domain",
            concrete("core.object")?,
            Some(TypedValue {
                value_type: concrete("core.object")?,
                value: DataValue::Object(Default::default()),
            }),
            vec![],
            ParameterEditorSpec::SemanticDomain,
        )?);
    }
    Ok(parameters)
}
