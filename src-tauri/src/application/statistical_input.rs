use crate::sci::api::computation::{
    CategoricalRole, StatisticalInput, StatisticalInputMappingError, StatisticalInputSource,
    StatisticalScalar, StatisticalValueKind,
};
use yss_data_contract::{CategoricalRole as PersistedCategoricalRole, DataValue};

pub fn statistical_input(
    source: StatisticalInputSource<'_>,
) -> Result<StatisticalInput, StatisticalInputMappingError> {
    let values = source
        .values
        .iter()
        .enumerate()
        .map(|(index, value)| map_value(index, value.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StatisticalInput::new(
        Box::from(source.name),
        values.into_boxed_slice(),
        source.categorical_role,
    ))
}

pub fn statistical_categorical_role(
    role: Option<&PersistedCategoricalRole>,
) -> Option<CategoricalRole> {
    role.map(|role| match role {
        PersistedCategoricalRole::General => CategoricalRole::General,
        PersistedCategoricalRole::Individual => CategoricalRole::Individual,
        PersistedCategoricalRole::Time => CategoricalRole::Time,
    })
}

fn map_value(
    index: usize,
    value: Option<&DataValue>,
) -> Result<Option<StatisticalScalar>, StatisticalInputMappingError> {
    match value {
        None | Some(DataValue::Null) => Ok(None),
        Some(DataValue::Int64(value)) => Ok(Some(StatisticalScalar::Numeric(*value as f64))),
        Some(DataValue::Float64(value)) if value.is_finite() => {
            Ok(Some(StatisticalScalar::Numeric(*value)))
        }
        Some(DataValue::Float64(_)) => {
            Err(StatisticalInputMappingError::NonFiniteNumeric { index })
        }
        Some(DataValue::String(value)) => {
            Ok(Some(StatisticalScalar::Category(value.as_str().into())))
        }
        Some(DataValue::Boolean(_)) => unsupported(index, StatisticalValueKind::Boolean),
        Some(DataValue::Array(_)) => unsupported(index, StatisticalValueKind::Array),
        Some(DataValue::Object(_)) => unsupported(index, StatisticalValueKind::Object),
        Some(DataValue::DataFrame(_)) => unsupported(index, StatisticalValueKind::DataFrame),
        Some(DataValue::DataSeries(_)) => unsupported(index, StatisticalValueKind::DataSeries),
        Some(DataValue::Struct { .. }) => unsupported(index, StatisticalValueKind::Struct),
    }
}

fn unsupported(
    index: usize,
    kind: StatisticalValueKind,
) -> Result<Option<StatisticalScalar>, StatisticalInputMappingError> {
    Err(StatisticalInputMappingError::UnsupportedValue { index, kind })
}

#[cfg(test)]
mod tests {
    use super::{statistical_categorical_role, statistical_input};
    use crate::sci::api::computation::{
        CategoricalRole, StatisticalInputMappingError, StatisticalInputSource, StatisticalScalar,
        StatisticalValueKind,
    };
    use std::collections::HashMap;
    use yss_data_contract::{
        CategoricalRole as PersistedCategoricalRole, DataSeriesValue, DataValue,
    };

    #[test]
    fn persisted_scalars_and_role_map_to_the_sci_input_contract() {
        let role = PersistedCategoricalRole::Time;
        let categorical_role = statistical_categorical_role(Some(&role));
        let values = [
            None,
            Some(DataValue::Null),
            Some(DataValue::Int64(7)),
            Some(DataValue::Float64(2.5)),
            Some(DataValue::String("group-a".to_owned())),
        ];

        let input = statistical_input(StatisticalInputSource {
            name: "series",
            values: &values,
            categorical_role,
        })
        .expect("supported persisted values must map to a statistical input");

        assert_eq!(input.name(), "series");
        assert_eq!(
            input.values(),
            [
                None,
                None,
                Some(StatisticalScalar::Numeric(7.0)),
                Some(StatisticalScalar::Numeric(2.5)),
                Some(StatisticalScalar::Category(Box::<str>::from("group-a"))),
            ]
        );
        assert_eq!(input.categorical_role(), Some(CategoricalRole::Time));
    }

    #[test]
    fn invalid_persisted_values_return_closed_mapping_errors() {
        let cases = [
            (
                DataValue::Float64(f64::INFINITY),
                StatisticalInputMappingError::NonFiniteNumeric { index: 0 },
            ),
            (
                DataValue::Boolean(true),
                StatisticalInputMappingError::UnsupportedValue {
                    index: 0,
                    kind: StatisticalValueKind::Boolean,
                },
            ),
            (
                DataValue::Array(Vec::new()),
                StatisticalInputMappingError::UnsupportedValue {
                    index: 0,
                    kind: StatisticalValueKind::Array,
                },
            ),
            (
                DataValue::Object(HashMap::new()),
                StatisticalInputMappingError::UnsupportedValue {
                    index: 0,
                    kind: StatisticalValueKind::Object,
                },
            ),
            (
                DataValue::DataFrame("frame".to_owned()),
                StatisticalInputMappingError::UnsupportedValue {
                    index: 0,
                    kind: StatisticalValueKind::DataFrame,
                },
            ),
            (
                DataValue::DataSeries(DataSeriesValue::new("series")),
                StatisticalInputMappingError::UnsupportedValue {
                    index: 0,
                    kind: StatisticalValueKind::DataSeries,
                },
            ),
            (
                DataValue::Struct {
                    type_key: "kind".to_owned(),
                    handle_id: "handle".to_owned(),
                },
                StatisticalInputMappingError::UnsupportedValue {
                    index: 0,
                    kind: StatisticalValueKind::Struct,
                },
            ),
        ];

        for (value, expected) in cases {
            let values = [Some(value)];
            assert_eq!(
                statistical_input(StatisticalInputSource {
                    name: "series",
                    values: &values,
                    categorical_role: None,
                }),
                Err(expected)
            );
        }
    }
}
