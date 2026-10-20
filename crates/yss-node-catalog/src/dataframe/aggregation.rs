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
            data_input(
                input,
                if series { "DataSeries" } else { "Source" },
                value_type,
            )?,
            data_output(
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
        vec![ParameterConstraint::ColumnNames],
    )
}
