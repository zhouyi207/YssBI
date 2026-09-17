use super::support::*;
use yss_node_protocol::*;

pub(super) fn register(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    const ID: &str = "yssbi.value.convert";
    fragment.add_node_messages(&NodeTextSpec {
        id: ID,
        title: "Type Conversion",
        zh_title: "类别转换",
        documentation: "Convert all seven scalar meanings while preserving scalar/series shape. Category domains and ordinal levels are explicit; calendar values retain wall time. Series stay lazy and nulls are preserved.",
        zh_documentation: "支持七种基础语义的转换，保持标量或数列结构。分类值域与等级顺序需明确配置，日期时间保留钟面时间；数列惰性执行并保留空值。",
        aliases: &["convert", "cast", "semantic conversion"],
        zh_aliases: &["转换", "类别转换", "类型转换", "语义转换"],
    })?;
    add_parameter_messages(
        fragment,
        ID,
        &[
            (
                "target_type",
                "Target Meaning",
                "目标语义",
                "Auto requires a unique target from connected downstream input ports. Connect a typed input or select a target explicitly if unresolved; conflicting requirements block execution.",
                "自动取下游输入端口约束的交集。未连线或目标不唯一时，请连接明确类型的下游或手动选择；约束冲突时无法执行。",
            ),
            (
                "numeric_mode",
                "Numeric Representation",
                "数值表示",
                "For Numeric targets: auto preserves existing numbers and parses text as real numbers; integer rejects fractions; real requires exact conversion.",
                "用于数值目标：自动保留现有数值表示，文本解析为实数；整数不允许小数；实数转换不得丢失精度。",
            ),
            (
                "semantic_domain",
                "Values and Levels",
                "值域与等级",
                "Declare original codes and labels. Ordinal levels run from low to high. Empty configuration inherits a compatible source domain.",
                "配置原始编码与标签；顺序目标按从低到高排列。留空时继承兼容的源值域。",
            ),
            (
                "datetime_kind",
                "Calendar Kind",
                "日期时间形式",
                "Auto preserves temporal inputs; text defaults to a date and time.",
                "自动保留已有时间表示，文本默认解析为日期时间；也可明确选择日期或时间。",
            ),
            (
                "datetime_precision",
                "Calendar Precision",
                "时间精度",
                "Precision for newly parsed values. Precision loss fails.",
                "新解析时间值使用的精度；不能静默截断更高精度。",
            ),
            (
                "datetime_format",
                "Calendar Format",
                "时间格式",
                "Empty uses ISO. Custom formats use strftime, e.g. %d/%m/%Y. Offsets are removed without shifting wall time.",
                "留空按 ISO 格式解析。自定义格式如 %d/%m/%Y；移除时区时保留原钟面时间。",
            ),
        ],
    )?;
    let mut parameters = Vec::new();
    for (key, default, options) in [
        (
            "target_type",
            "auto",
            std::iter::once("auto")
                .chain(SemanticType::ALL.into_iter().map(SemanticType::type_id))
                .collect(),
        ),
        ("numeric_mode", "auto", vec!["auto", "integer", "real"]),
        (
            "datetime_kind",
            "auto",
            vec!["auto", "date", "time", "datetime"],
        ),
        (
            "datetime_precision",
            "microseconds",
            vec!["seconds", "milliseconds", "microseconds", "nanoseconds"],
        ),
    ] {
        parameters.push(parameter(
            ID,
            key,
            concrete("core.text")?,
            Some(ParameterValue {
                value_type: concrete("core.text")?,
                value: Value::String(default.into()),
            }),
            vec![ParameterConstraint::OneOf(
                options
                    .into_iter()
                    .map(|value| Value::String(value.into()))
                    .collect(),
            )],
            ParameterEditorSpec::Select,
        )?);
    }
    parameters.push(parameter(
        ID,
        "datetime_format",
        concrete("core.text")?,
        Some(ParameterValue {
            value_type: concrete("core.text")?,
            value: Value::String("".into()),
        }),
        vec![ParameterConstraint::Length {
            min: None,
            max: Some(256),
        }],
        ParameterEditorSpec::Text { multiline: false },
    )?);
    parameters.push(parameter(
        ID,
        "semantic_domain",
        concrete("core.object")?,
        Some(ParameterValue {
            value_type: concrete("core.object")?,
            value: Value::Object(Default::default()),
        }),
        vec![],
        ParameterEditorSpec::SemanticDomain,
    )?);
    let types = TypeExpr::Union(
        SemanticType::ALL
            .into_iter()
            .map(SemanticType::type_id)
            .map(|kind| Ok([concrete(kind)?, data_series(kind)?]))
            .collect::<Result<Vec<_>, BuiltinAssemblyError>>()?
            .into_iter()
            .flatten()
            .collect(),
    );
    let mut input = data_port("input", "Input", PortDirection::Input, types.clone())?;
    input.consumption = Some(InputConsumption::Streaming);
    let mut output = data_port("output", "Output", PortDirection::Output, types)?;
    output.production = Some(OutputProduction::Streaming);
    let mut protocol = protocol(
        ID,
        "conversion",
        vec![input, output],
        vec![],
        parameters,
        pure(),
    )?;
    protocol.typing = NodeTypingSpec::ShapePreservingConversion {
        input: semantic("input", PortKey::new)?,
        parameter: semantic("target_type", ParameterKey::new)?,
        output: semantic("output", PortKey::new)?,
    };
    fragment.nodes.push(leaf(protocol, ID));
    Ok(())
}
