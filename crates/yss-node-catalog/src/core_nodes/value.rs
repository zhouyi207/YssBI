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
    for (id, target) in [
        ("yssbi.value.to_numeric", SemanticType::Numeric),
        ("yssbi.value.to_text", SemanticType::Text),
        ("yssbi.value.to_categorical", SemanticType::Categorical),
        ("yssbi.value.to_ordinal", SemanticType::Ordinal),
        ("yssbi.value.to_binary", SemanticType::Binary),
        ("yssbi.value.to_datetime", SemanticType::Datetime),
        ("yssbi.value.to_identifier", SemanticType::Identifier),
    ] {
        let parameters = conversion_parameters(id, target)?;
        let input = data_port(
            "input",
            "Input",
            PortDirection::Input,
            input_types.clone(),
            PortCardinality::Declared,
            LiteralPolicy::Allowed,
        )?;
        let output = data_port(
            "output",
            "Output",
            PortDirection::Output,
            TypeExpr::Union(vec![
                concrete(target.type_id())?,
                data_series(target.type_id())?,
            ]),
            PortCardinality::Declared,
            LiteralPolicy::Allowed,
        )?;
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
    id: &str,
    target: SemanticType,
) -> Result<Vec<Parameter>, BuiltinAssemblyError> {
    let mut parameters = Vec::new();
    for (kind, key, default, options) in [
        (
            SemanticType::Numeric,
            "numeric_mode",
            "auto",
            &["auto", "integer", "real"][..],
        ),
        (
            SemanticType::Datetime,
            "datetime_kind",
            "auto",
            &["auto", "date", "time", "datetime"][..],
        ),
        (
            SemanticType::Datetime,
            "datetime_precision",
            "microseconds",
            &["seconds", "milliseconds", "microseconds", "nanoseconds"][..],
        ),
    ] {
        if kind != target {
            continue;
        }
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
    if matches!(
        target,
        SemanticType::Categorical | SemanticType::Ordinal | SemanticType::Binary
    ) {
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
