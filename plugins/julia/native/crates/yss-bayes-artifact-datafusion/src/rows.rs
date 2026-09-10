use super::{BayesArtifactReadError, PosteriorPredictiveRow, PosteriorPredictiveSummary};
use arrow::array::{
    Array, Float64Array, Int64Array, LargeStringArray, StringArray, StringViewArray,
};
use arrow::datatypes::DataType;
use arrow::record_batch::RecordBatch;

pub(crate) const PREDICTIVE_COLUMNS: &[&str] = &[
    "observation",
    "response_transform",
    "observed_model",
    "mean_model",
    "q025_model",
    "q975_model",
    "observed_original",
    "mean_original",
    "q025_original",
    "q975_original",
];

fn string(array: &dyn Array, row: usize) -> Option<&str> {
    if array.is_null(row) {
        return None;
    }
    if let Some(array) = array.as_any().downcast_ref::<StringArray>() {
        Some(array.value(row))
    } else if let Some(array) = array.as_any().downcast_ref::<LargeStringArray>() {
        Some(array.value(row))
    } else {
        array
            .as_any()
            .downcast_ref::<StringViewArray>()
            .map(|array| array.value(row))
    }
}
fn is_string(array: &dyn Array) -> bool {
    matches!(
        array.data_type(),
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View
    )
}
fn identity(array: &Int64Array, row: usize) -> Option<usize> {
    (!array.is_null(row))
        .then(|| usize::try_from(array.value(row)).ok())
        .flatten()
}
fn number(array: &Float64Array, row: usize) -> Option<f64> {
    (!array.is_null(row) && array.value(row).is_finite()).then(|| array.value(row))
}
fn column<'a, T: Array + 'static>(batch: &'a RecordBatch, name: &str) -> Option<&'a T> {
    batch.column_by_name(name)?.as_any().downcast_ref::<T>()
}

pub(crate) fn samples(
    batch: &RecordBatch,
    mut visitor: impl FnMut(&str, usize, usize, f64) -> Result<(), BayesArtifactReadError>,
) -> Result<(), BayesArtifactReadError> {
    let invalid = super::samples_invalid;
    let parameters = batch.column_by_name("parameter").ok_or_else(invalid)?;
    if !is_string(parameters.as_ref()) {
        return Err(invalid());
    }
    let chains = column::<Int64Array>(batch, "chain").ok_or_else(invalid)?;
    let draws = column::<Int64Array>(batch, "draw").ok_or_else(invalid)?;
    let values = column::<Float64Array>(batch, "value").ok_or_else(invalid)?;
    for row in 0..batch.num_rows() {
        visitor(
            string(parameters.as_ref(), row).ok_or_else(invalid)?,
            identity(chains, row).ok_or_else(invalid)?,
            identity(draws, row).ok_or_else(invalid)?,
            number(values, row).ok_or_else(invalid)?,
        )?;
    }
    Ok(())
}

pub(crate) fn predictive(
    batch: &RecordBatch,
    mut visitor: impl FnMut(&str, PosteriorPredictiveRow) -> Result<(), BayesArtifactReadError>,
) -> Result<(), BayesArtifactReadError> {
    let invalid = super::posterior_predictive_invalid;
    let observations = column::<Int64Array>(batch, "observation").ok_or_else(invalid)?;
    let transforms = batch
        .column_by_name("response_transform")
        .ok_or_else(invalid)?;
    if !is_string(transforms.as_ref()) {
        return Err(invalid());
    }
    let columns = PREDICTIVE_COLUMNS[2..]
        .iter()
        .map(|name| column::<Float64Array>(batch, name).ok_or_else(invalid))
        .collect::<Result<Vec<_>, _>>()?;
    for row in 0..batch.num_rows() {
        let summary = |start: usize| -> Result<PosteriorPredictiveSummary, BayesArtifactReadError> {
            Ok(PosteriorPredictiveSummary {
                observed: number(columns[start], row).ok_or_else(invalid)?,
                mean: number(columns[start + 1], row).ok_or_else(invalid)?,
                q025: number(columns[start + 2], row).ok_or_else(invalid)?,
                q975: number(columns[start + 3], row).ok_or_else(invalid)?,
            })
        };
        visitor(
            string(transforms.as_ref(), row).ok_or_else(invalid)?,
            PosteriorPredictiveRow {
                observation: identity(observations, row).ok_or_else(invalid)?,
                model: summary(0)?,
                original: summary(4)?,
            },
        )?;
    }
    Ok(())
}
