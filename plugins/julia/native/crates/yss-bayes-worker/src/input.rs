use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BayesCategoricalRole {
    General,
    Individual,
    Time,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatisticalScalar {
    Numeric(f64),
    Category(Box<str>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatisticalInput {
    name: Box<str>,
    values: Box<[Option<StatisticalScalar>]>,
    categorical_role: Option<BayesCategoricalRole>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StatisticalInputValidationError {
    #[error("statistical input name is blank")]
    BlankName,
    #[error("statistical input contains a non-finite numeric value")]
    NonFiniteNumeric { index: usize },
}

impl StatisticalInput {
    pub fn try_new(
        name: Box<str>,
        values: Box<[Option<StatisticalScalar>]>,
        categorical_role: Option<BayesCategoricalRole>,
    ) -> Result<Self, StatisticalInputValidationError> {
        if name.trim().is_empty() {
            return Err(StatisticalInputValidationError::BlankName);
        }
        if let Some(index) = values.iter().position(
            |value| matches!(value, Some(StatisticalScalar::Numeric(number)) if !number.is_finite()),
        ) {
            return Err(StatisticalInputValidationError::NonFiniteNumeric { index });
        }
        Ok(Self {
            name,
            values,
            categorical_role,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn values(&self) -> &[Option<StatisticalScalar>] {
        &self.values
    }

    pub fn categorical_role(&self) -> Option<BayesCategoricalRole> {
        self.categorical_role
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistical_inputs_can_only_be_constructed_from_named_finite_values() {
        assert_eq!(
            StatisticalInput::try_new("  ".into(), Box::new([]), None),
            Err(StatisticalInputValidationError::BlankName)
        );
        assert_eq!(
            StatisticalInput::try_new(
                "series".into(),
                Box::new([Some(StatisticalScalar::Numeric(f64::INFINITY))]),
                None,
            ),
            Err(StatisticalInputValidationError::NonFiniteNumeric { index: 0 })
        );

        let input = StatisticalInput::try_new(
            "series".into(),
            Box::new([
                None,
                Some(StatisticalScalar::Numeric(2.5)),
                Some(StatisticalScalar::Category("group-a".into())),
            ]),
            Some(BayesCategoricalRole::Time),
        )
        .expect("named finite statistical input must be constructible");
        assert_eq!(input.name(), "series");
        assert_eq!(input.categorical_role(), Some(BayesCategoricalRole::Time));
    }
}
