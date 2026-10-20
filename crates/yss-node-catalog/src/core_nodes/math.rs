use super::support::*;
use yss_node_protocol::*;

const ARITHMETIC_OPERATORS: &[&str] = &["power", "log", "add", "subtract", "multiply", "divide"];

const UNARY_FUNCTIONS: &[&str] = &["ln", "log2", "log10", "sqrt", "square"];

pub(super) fn register(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    register_constants(fragment)?;
    for operation in ARITHMETIC_OPERATORS {
        register_arithmetic_operator(fragment, operation)?;
    }
    for operation in UNARY_FUNCTIONS {
        register_unary(fragment, operation)?;
    }
    Ok(())
}

fn register_constants(fragment: &mut ProviderFragment) -> Result<(), BuiltinAssemblyError> {
    for id in ["yssbi.constant.pi", "yssbi.constant.e"] {
        let protocol = protocol(
            id,
            "constants",
            vec![data_port(
                "value",
                "Value",
                PortDirection::Output,
                concrete("core.numeric")?,
                PortCardinality::Declared,
                LiteralPolicy::Allowed,
            )?],
            vec![],
            vec![],
            pure(),
        )?;
        fragment.nodes.push(leaf(protocol, id));
    }
    Ok(())
}

fn register_arithmetic_operator(
    fragment: &mut ProviderFragment,
    operation: &str,
) -> Result<(), BuiltinAssemblyError> {
    let id = format!("yssbi.numeric.{operation}");
    let numeric = numeric_value_type()?;
    let (left_title, right_title) = match operation {
        "power" => ("Base", "Exponent"),
        "log" => ("Value", "Base"),
        _ => ("Left", "Right"),
    };
    let operands = if operation == "add" {
        data_port(
            "operands",
            "Operands",
            PortDirection::Input,
            numeric.clone(),
            PortCardinality::UserCreated { min: 2, max: None },
            LiteralPolicy::Allowed,
        )?
    } else {
        data_port(
            "left",
            left_title,
            PortDirection::Input,
            numeric.clone(),
            PortCardinality::Declared,
            LiteralPolicy::Allowed,
        )?
    };
    let mut ports = vec![operands];
    if operation != "add" {
        ports.push(data_port(
            "right",
            right_title,
            PortDirection::Input,
            numeric,
            PortCardinality::Declared,
            LiteralPolicy::Allowed,
        )?);
    }
    let result_type = if matches!(operation, "divide" | "power" | "log") {
        float_value_type()?
    } else {
        numeric_value_type()?
    };
    ports.push(data_port(
        "result",
        "Result",
        PortDirection::Output,
        result_type,
        PortCardinality::Declared,
        LiteralPolicy::Allowed,
    )?);
    let mut protocol = protocol(&id, "numeric", ports, vec![], vec![], pure())?;
    protocol.typing = NodeTypingSpec::NumericFold {
        inputs: if operation == "add" {
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
    fragment.nodes.push(leaf(protocol, &id));
    Ok(())
}

fn register_unary(
    fragment: &mut ProviderFragment,
    operation: &str,
) -> Result<(), BuiltinAssemblyError> {
    let id = format!("yssbi.numeric.{operation}");
    let numeric = numeric_value_type()?;
    let output = TypeExpr::Union(vec![concrete("core.numeric")?, numeric_data_series_type()]);
    let mut protocol = protocol(
        &id,
        "numeric",
        vec![
            data_port(
                "input",
                "Input",
                PortDirection::Input,
                numeric,
                PortCardinality::Declared,
                LiteralPolicy::Allowed,
            )?,
            data_port(
                "result",
                "Result",
                PortDirection::Output,
                output,
                PortCardinality::Declared,
                LiteralPolicy::Allowed,
            )?,
        ],
        vec![],
        vec![],
        pure(),
    )?;
    protocol.typing = NodeTypingSpec::ShapePreservingNumeric {
        input: semantic("input", PortKey::new)?,
        output: semantic("result", PortKey::new)?,
    };
    fragment.nodes.push(leaf(protocol, &id));
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
        numeric_data_series_type(),
    ]))
}
