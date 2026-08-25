use super::support::*;
use crate::node_system::protocol::*;

#[derive(Clone, Copy)]
struct MathSpec {
    operation: &'static str,
    title: &'static str,
    zh_title: &'static str,
    aliases: &'static [&'static str],
    zh_aliases: &'static [&'static str],
}

const SERIES_OPERATORS: &[MathSpec] = &[
    MathSpec {
        operation: "add",
        title: "Add DataSeries",
        zh_title: "数据序列加法",
        aliases: &["series add", "element-wise addition", "+"],
        zh_aliases: &["序列相加", "逐元素加法", "+"],
    },
    MathSpec {
        operation: "subtract",
        title: "Subtract DataSeries",
        zh_title: "数据序列减法",
        aliases: &["series subtract", "element-wise subtraction", "-"],
        zh_aliases: &["序列相减", "逐元素减法", "-"],
    },
    MathSpec {
        operation: "multiply",
        title: "Multiply DataSeries",
        zh_title: "数据序列乘法",
        aliases: &["series multiply", "element-wise multiplication", "*"],
        zh_aliases: &["序列相乘", "逐元素乘法", "*"],
    },
    MathSpec {
        operation: "divide",
        title: "Divide DataSeries",
        zh_title: "数据序列除法",
        aliases: &["series divide", "element-wise division", "/"],
        zh_aliases: &["序列相除", "逐元素除法", "/"],
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
        operation: "exp",
        title: "Exponential",
        zh_title: "指数函数",
        aliases: &["exp", "exponential", "e power"],
        zh_aliases: &["指数", "指数函数", "exp"],
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
    for spec in SERIES_OPERATORS {
        register_series_operator(fragment, *spec)?;
    }
    for spec in UNARY_FUNCTIONS {
        register_unary(fragment, *spec)?;
    }
    Ok(())
}

fn register_series_operator(
    fragment: &mut ProviderFragment,
    spec: MathSpec,
) -> Result<(), BuiltinAssemblyError> {
    let id = leak(format!("yssbi.numeric.series.{}", spec.operation));
    fragment.add_node_messages(&NodeTextSpec {
        id,
        title: spec.title,
        zh_title: spec.zh_title,
        description: "Applies deterministic element-wise numeric operations to materialized DataSeries values.",
        zh_description: "对完全物化的 DataSeries 值执行确定性的逐元素数值运算。",
        documentation: "Scalar inputs are broadcast to the DataSeries length. DataSeries operands must have equal lengths.",
        zh_documentation: "标量输入会广播到 DataSeries 长度；多个 DataSeries 操作数的长度必须一致。",
        aliases: spec.aliases,
        zh_aliases: spec.zh_aliases,
    })?;
    let numeric = numeric_value_type()?;
    let operands = if spec.operation == "add" {
        data_port_with_instances(
            "operands",
            "Operands",
            PortDirection::Input,
            numeric.clone(),
            PortInstances::UserCreated { min: 2, max: None },
        )?
    } else {
        data_port("left", "Left", PortDirection::Input, numeric.clone())?
    };
    let mut ports = vec![operands];
    if spec.operation != "add" {
        ports.push(data_port("right", "Right", PortDirection::Input, numeric)?);
    }
    let result_type = if spec.operation == "divide" {
        data_series("core.float64")?
    } else {
        numeric_data_series_type()
    };
    ports.push(data_port(
        "result",
        "Result",
        PortDirection::Output,
        result_type,
    )?);
    fragment.nodes.push(leaf(
        protocol(id, "numeric", ports, vec![], vec![], vec![], pure())?,
        id,
    ));
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
        description: "Applies a unary Float64 function to a scalar or each DataSeries element.",
        zh_description: "对 Float64 标量或 DataSeries 的每个元素应用一元函数。",
        documentation: "The output shape matches the input shape. Non-finite results are rejected by the runtime kernel.",
        zh_documentation: "输出形状与输入一致；运行时 kernel 会拒绝非有限结果。",
        aliases: spec.aliases,
        zh_aliases: spec.zh_aliases,
    })?;
    let numeric = numeric_value_type()?;
    let output = TypeExpr::Union(vec![
        concrete("core.float64")?,
        data_series("core.float64")?,
    ]);
    fragment.nodes.push(leaf(
        protocol(
            id,
            "numeric",
            vec![
                data_port("input", "Input", PortDirection::Input, numeric)?,
                data_port("result", "Result", PortDirection::Output, output)?,
            ],
            vec![],
            vec![],
            vec![],
            pure(),
        )?,
        id,
    ));
    Ok(())
}

fn numeric_value_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    Ok(TypeExpr::Union(vec![
        concrete("core.int64")?,
        concrete("core.float64")?,
        data_series("core.int64")?,
        data_series("core.float64")?,
    ]))
}
