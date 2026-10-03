//! Shared statistical label-series types and fixed numeric table ports.
use super::*;

pub(super) fn fixed_numeric_table(
    key: &'static str,
    title: &'static str,
    names: &[&str],
) -> Result<PortSpec, BuiltinAssemblyError> {
    let mut port = data_output(key, title, concrete("tabular.dataframe")?)?;
    port.schema = Some(SchemaExpr::Fixed {
        fields: names
            .iter()
            .map(|name| SchemaField {
                name: SchemaColumnRef((*name).into()),
                scalar_type: RelationalScalarType::Known(yss_data_contract::SemanticType::Numeric),
                lineage: None,
            })
            .collect(),
    });
    Ok(port)
}

pub(super) fn label_series() -> Result<TypeExpr, BuiltinAssemblyError> {
    let members = [
        "core.numeric",
        "core.categorical",
        "core.ordinal",
        "core.binary",
        "core.text",
        "core.identifier",
    ]
    .iter()
    .map(|id| concrete(id).map(data_series_type))
    .collect::<Result<Vec<_>, _>>()?;
    normalize_type_expr(TypeExpr::Union(members)).map_err(|error| {
        BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
            context: "statistical group input",
            value: error.to_string().into(),
        }
    })
}

pub(super) fn fitted_regression_type() -> Result<TypeExpr, BuiltinAssemblyError> {
    normalize_type_expr(TypeExpr::Union(
        [
            "statistics.model.linear",
            "statistics.model.logit",
            "statistics.model.probit",
        ]
        .into_iter()
        .map(concrete)
        .collect::<Result<Vec<_>, _>>()?,
    ))
    .map_err(|e| BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
        context: "retained regression model",
        value: e.to_string().into(),
    })
}
