use std::collections::BTreeSet;

use arrow::{array::Array, datatypes::Field};
use yss_data_contract::{ConversionDomain, SemanticConversion, SemanticType, SemanticValue};

use crate::{TabularArrowError, column_semantic};

/// One complete, unordered codebook, collected across all input batches.
#[derive(Default)]
pub struct InferredCategoricalDomain {
    codes: BTreeSet<String>,
    label_bytes: usize,
}

impl InferredCategoricalDomain {
    pub fn for_conversion(
        source: &Field,
        spec: &SemanticConversion,
    ) -> Result<Option<Self>, TabularArrowError> {
        if spec.target != SemanticType::Categorical || !spec.domain.values.is_empty() {
            return Ok(None);
        }
        if !spec.domain.is_valid() {
            return Err(TabularArrowError::InvalidValue);
        }
        Ok(column_semantic(source)?
            .values
            .is_empty()
            .then(Self::default))
    }

    pub fn extend(&mut self, array: &dyn Array) -> Result<(), TabularArrowError> {
        super::finite(array)?;
        for code in super::strings(array)?.iter().flatten() {
            if self.codes.contains(code) {
                continue;
            }
            let bytes = code
                .len()
                .checked_mul(2)
                .and_then(|bytes| self.label_bytes.checked_add(bytes))
                .ok_or(TabularArrowError::InvalidValue)?;
            if self.codes.len() == ConversionDomain::MAX_VALUES
                || bytes > ConversionDomain::MAX_BYTES
            {
                return Err(TabularArrowError::InvalidValue);
            }
            self.label_bytes = bytes;
            self.codes.insert(code.into());
        }
        Ok(())
    }

    /// Budget the codebook together with its prepared conversion and metadata copies.
    pub fn retained_bytes(&self) -> usize {
        (self.label_bytes + self.codes.len() * size_of::<SemanticValue>()) * 4
    }

    pub fn finish(self) -> ConversionDomain {
        ConversionDomain {
            values: self
                .codes
                .into_iter()
                .map(|value| SemanticValue {
                    label: value.clone(),
                    value,
                })
                .collect(),
            positive_value: None,
        }
    }
}
