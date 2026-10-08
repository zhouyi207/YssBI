//! Shared statistical series types and fixed table ports.
use super::*;
use yss_data_contract::SemanticType;

pub(super) fn fixed_numeric_table(
    key: &'static str,
    title: &'static str,
    names: &[&str],
) -> Result<PortSpec, BuiltinAssemblyError> {
    fixed_table(
        key,
        title,
        names.iter().map(|name| (*name, SemanticType::Numeric)),
    )
}

pub(super) fn fixed_table<'a>(
    key: &'static str,
    title: &'static str,
    columns: impl IntoIterator<Item = (&'a str, SemanticType)>,
) -> Result<PortSpec, BuiltinAssemblyError> {
    let mut port = data_output(key, title, concrete("tabular.dataframe")?)?;
    port.schema = Some(SchemaExpr::Fixed {
        fields: columns
            .into_iter()
            .map(|(name, scalar_type)| SchemaField {
                name: SchemaColumnRef(name.into()),
                scalar_type: RelationalScalarType::Known(scalar_type),
                lineage: None,
            })
            .collect(),
    });
    Ok(port)
}

pub(super) fn numeric_or_binary_series() -> Result<TypeExpr, BuiltinAssemblyError> {
    normalize_type_expr(TypeExpr::Union(vec![
        series_type()?,
        data_series_type(concrete("core.binary")?),
    ]))
    .map_err(
        |error| BuiltinAssemblyError::UnsupportedBuiltinConfiguration {
            context: "numeric or binary statistical input",
            value: error.to_string().into(),
        },
    )
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
