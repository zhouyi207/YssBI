use super::support::*;
use yss_node_protocol::*;

#[derive(Clone, Copy)]
struct MathSpec {
    operation: &'static str,
    title: &'static str,
    zh_title: &'static str,
    aliases: &'static [&'static str],
    zh_aliases: &'static [&'static str],
}

const ARITHMETIC_OPERATORS: &[MathSpec] = &[
    MathSpec {
        operation: "power",
        title: "Power",
        zh_title: "幂",
        aliases: &["power", "pow", "exponent", "^"],
        zh_aliases: &["幂", "乘方", "指数", "次方"],
    },
    MathSpec {
        operation: "log",
        title: "Logarithm",
        zh_title: "对数",
        aliases: &["log", "logarithm", "base"],
        zh_aliases: &["对数", "底数", "换底"],
    },
    MathSpec {
        operation: "add",
        title: "Add",
        zh_title: "加法",
        aliases: &["add", "plus", "sum", "series add", "+"],
        zh_aliases: &["加法", "相加", "求和", "序列相加", "+"],
    },
    MathSpec {
        operation: "subtract",
        title: "Subtract",
        zh_title: "减法",
        aliases: &["subtract", "minus", "difference", "series subtract", "-"],
        zh_aliases: &["减法", "相减", "差", "序列相减", "-"],
    },
    MathSpec {
        operation: "multiply",
        title: "Multiply",
        zh_title: "乘法",
        aliases: &["multiply", "times", "product", "series multiply", "*"],
        zh_aliases: &["乘法", "相乘", "积", "序列相乘", "*"],
    },
    MathSpec {
        operation: "divide",
        title: "Divide",
        zh_title: "除法",
        aliases: &["divide", "quotient", "series divide", "/"],
        zh_aliases: &["除法", "相除", "商", "序列相除", "/"],
    },
];

const UNARY_FUNCTIONS: &[MathSpec] = &[
    MathSpec {
        operation: "ln",
        title: "Natural Logarithm",
        zh_title: "自然对数",
        aliases: &["ln", "natural log", "log e"],
        zh_aliases: &["自然对数", "ln"],
    },
    MathSpec {
        operation: "log2",
        title: "Base-2 Logarithm",
        zh_title: "以 2 为底的对数",
        aliases: &["log2", "binary logarithm"],
        zh_aliases: &["二进制对数", "log2"],
    },
    MathSpec {
        operation: "log10",
        title: "Base-10 Logarithm",
        zh_title: "常用对数",
        aliases: &["log10", "common logarithm"],
        zh_aliases: &["常用对数", "log10"],
    },
    MathSpec {
        operation: "sqrt",
        title: "Square Root",
        zh_title: "平方根",
        aliases: &["sqrt", "square root", "root"],
        zh_aliases: &["平方根", "开方", "sqrt"],
    },
    MathSpec {
        operation: "square",
        title: "Square",
        zh_title: "平方",
        aliases: &["square", "power two", "x squared"],
        zh_aliases: &["平方", "二次方"],
    },
];

pub(super) fn register(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    register_constants(fragment)?;
    for spec in ARITHMETIC_OPERATORS {
        register_arithmetic_operator(fragment, *spec)?;
    }
    for spec in UNARY_FUNCTIONS {
        register_unary(fragment, *spec)?;
    }
    Ok(())
}

fn register_constants(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for spec in [
        NodeTextSpec {
            id: "yssbi.constant.pi",
            title: "Pi (π)",
            zh_title: "π（圆周率）",
            documentation: "The fixed mathematical constant pi, represented as Float64.",
            zh_documentation: "输出圆周率 π 的 Float64 近似值，无需输入或配置。",
            aliases: &["pi", "π", "circle constant"],
            zh_aliases: &["pi", "π", "圆周率"],
        },
        NodeTextSpec {
            id: "yssbi.constant.e",
            title: "Euler's Number (e)",
            zh_title: "e（自然常数）",
            documentation: "The fixed base of the natural logarithm, represented as Float64.",
            zh_documentation: "输出自然常数 e 的 Float64 近似值，可用作幂和对数的底数。",
            aliases: &["e", "Euler's number", "natural logarithm base"],
            zh_aliases: &["e", "自然常数", "自然对数底数"],
        },
    ] {
        fragment.add_node_messages(&spec)?;
        let protocol = protocol(
            spec.id,
            "constants",
            vec![data_port(
                "value",
                "Value",
                PortDirection::Output,
                concrete("core.numeric")?,
            )?],
            vec![],
            vec![],
            pure(),
        )?;
        fragment.nodes.push(leaf(protocol, spec.id));
    }
    Ok(())
}

fn register_arithmetic_operator(
    fragment: &mut ProviderFragment,
    spec: MathSpec,
) -> Result<(), BuiltinAssemblyError> {
    let id = leak(format!("yssbi.numeric.{}", spec.operation));
    fragment.add_node_messages(&NodeTextSpec {
        id,
        title: spec.title,
        zh_title: spec.zh_title,
        documentation: "Scalar-only inputs produce a scalar. If any input is a DataSeries, scalar inputs are broadcast and the result is a DataSeries. Int64 widens to Float64 when required.",
        zh_documentation: "输入全为标量时输出标量；任一输入为 DataSeries 时，标量会广播且结果为 DataSeries。需要时 Int64 会提升为 Float64。",
        aliases: spec.aliases,
        zh_aliases: spec.zh_aliases,
    })?;
    let numeric = numeric_value_type()?;
    let (left_title, right_title) = match spec.operation {
        "power" => ("Base", "Exponent"),
        "log" => ("Value", "Base"),
        _ => ("Left", "Right"),
    };
    let operands = if spec.operation == "add" {
        data_port_with_cardinality(
            "operands",
            "Operands",
            PortDirection::Input,
            numeric.clone(),
            PortCardinality::UserCreated { min: 2, max: None },
        )?
    } else {
        data_port("left", left_title, PortDirection::Input, numeric.clone())?
    };
    let mut ports = vec![operands];
    if spec.operation != "add" {
        ports.push(data_port(
            "right",
            right_title,
            PortDirection::Input,
            numeric,
        )?);
    }
    let result_type = if matches!(spec.operation, "divide" | "power" | "log") {
        float_value_type()?
    } else {
        numeric_value_type()?
    };
    ports.push(data_port(
        "result",
        "Result",
        PortDirection::Output,
        result_type,
    )?);
    if matches!(spec.operation, "power" | "log") {
        for port in &mut ports {
            if port.direction == PortDirection::Input {
                port.consumption = Some(InputConsumption::Streaming);
            } else {
                port.production = Some(OutputProduction::Streaming);
            }
        }
    }
    let mut protocol = protocol(id, "numeric", ports, vec![], vec![], pure())?;
    protocol.typing = NodeTypingSpec::NumericFold {
        inputs: if spec.operation == "add" {
            Box::new([PortSelector::AllInstances(semantic(
                "operands",
                PortKey::new,
            )?)])
        } else {
            Box::new([
                PortSelector::Declared(semantic("left", PortKey::new)?),
                PortSelector::Declared(semantic("right", PortKey::new)?),
            ])
        },
        output: semantic("result", PortKey::new)?,
        shape: ShapeRule::AnySeriesElseScalar,
    };
    fragment.nodes.push(leaf(protocol, id));
    Ok(())
}

fn register_unary(
    fragment: &mut ProviderFragment,
    spec: MathSpec,
) -> Result<(), BuiltinAssemblyError> {
    let id = leak(format!("yssbi.numeric.{}", spec.operation));
    fragment.add_node_messages(&NodeTextSpec {
        id,
        title: spec.title,
        zh_title: spec.zh_title,
        documentation: "The output shape matches the input shape. Non-finite results are rejected by the runtime kernel.",
        zh_documentation: "输出形状与输入一致；运行时 kernel 会拒绝非有限结果。",
        aliases: spec.aliases,
        zh_aliases: spec.zh_aliases,
    })?;
    let numeric = numeric_value_type()?;
    let output = TypeExpr::Union(vec![
        concrete("core.numeric")?,
        data_series("core.numeric")?,
    ]);
    let mut protocol = protocol(
        id,
        "numeric",
        vec![
            data_port("input", "Input", PortDirection::Input, numeric)?,
            data_port("result", "Result", PortDirection::Output, output)?,
        ],
        vec![],
        vec![],
        pure(),
    )?;
    protocol.typing = NodeTypingSpec::ShapePreservingNumeric {
        input: semantic("input", PortKey::new)?,
        output: semantic("result", PortKey::new)?,
    };
    for port in &mut protocol.interface.ports {
        if port.direction == PortDirection::Input {
            port.consumption = Some(InputConsumption::Streaming);
        } else {
            port.production = Some(OutputProduction::Streaming);
        }
    }
    fragment.nodes.push(leaf(protocol, id));
    Ok(())
}

fn numeric_value_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    let numeric = TypeExpr::Class(semantic(NUMERIC_TYPE_CLASS_ID, TypeClassId::new)?);
    Ok(TypeExpr::Union(vec![
        numeric.clone(),
        TypeExpr::Applied {
            constructor: semantic(DATA_SERIES_CONSTRUCTOR_ID, TypeConstructorId::new)?,
            arguments: vec![numeric],
        },
    ]))
}

fn float_value_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(TypeExpr::Union(vec![
        concrete("core.numeric")?,
        data_series("core.numeric")?,
    ]))
}
