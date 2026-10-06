//! Shared result protocol mapping for desktop IPC and Harness adapters.
use crate::graph::results::report::LinearRegressionReportProjection;
use crate::graph::results::{ResultQueryApplicationError, ResultValueProjection};
use yss_graph_execution::result::ResultReference;
use yss_node_kernel::RuntimeValue;

pub(crate) const MAX_INLINE_RESULT_JSON_BYTES: usize = 64 * 1024;

pub(crate) fn report_to_json(projection: LinearRegressionReportProjection) -> serde_json::Value {
    let reference = serde_json::json!({
        "executionSessionId": projection.reference.execution_session_id.as_uuid().to_string(),
        "resultId": projection.reference.result_id.get().to_string(),
    });
    report_to_json_with_refs(
        projection,
        reference,
        &|part, count| serde_json::json!({"kind": "tableRef", "part": part, "rowCount": count}),
    )
}

pub(crate) fn report_to_json_with_refs(
    projection: LinearRegressionReportProjection,
    reference: serde_json::Value,
    table: &dyn Fn(&str, usize) -> serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "summary": projection.summary,
        "title": projection.title,
        "endog_name": projection.endog_name,
        "paramNames": projection.param_names,
        "resultRef": reference,
        "presentation": crate::graph::results::report::presentation::data(&projection.model, projection.condition_number),
        "coefficients": table("coefficients", projection.coefficient_count),
        "observations": table("observations", projection.observation_count),
    })
}

pub(crate) fn query_result_json(
    application: &crate::ApplicationState,
    reference: ResultReference,
) -> Result<Option<serde_json::Value>, ResultQueryApplicationError> {
    let encoded = application
        .query_result_projection(reference)?
        .map(encode_inline_projection)
        .transpose();
    if application.query_result(reference)?.is_none() {
        return Ok(None);
    }
    encoded
}

fn encode_inline_projection(
    projection: ResultValueProjection,
) -> Result<serde_json::Value, ResultQueryApplicationError> {
    let value = match projection {
        ResultValueProjection::Value(value) => runtime_value_to_json(&value)?,
        ResultValueProjection::LinearModel(model) => serde_json::to_value(model)
            .map_err(|_| ResultQueryApplicationError::UnrepresentableValue)?,
        ResultValueProjection::LinearReport(report) => report_to_json(*report),
    };
    bound_inline_json(value)
}

pub(crate) fn bound_inline_json(
    value: serde_json::Value,
) -> Result<serde_json::Value, ResultQueryApplicationError> {
    if !json_fits_budget(&value, MAX_INLINE_RESULT_JSON_BYTES) {
        return Err(ResultQueryApplicationError::PageTooLarge);
    }
    Ok(value)
}

pub(crate) fn json_fits_budget(value: &impl serde::Serialize, maximum_bytes: usize) -> bool {
    // Count encoded bytes without allocating a second, potentially large byte buffer.
    struct Budget(usize);
    impl std::io::Write for Budget {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_sub(bytes.len())
                .ok_or_else(|| std::io::Error::other("JSON byte budget exceeded"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Budget(maximum_bytes), value).is_ok()
}

pub(crate) fn runtime_value_to_json(
    value: &RuntimeValue,
) -> Result<serde_json::Value, ResultQueryApplicationError> {
    Ok(match value {
        RuntimeValue::Annotated(value) => runtime_value_to_json(value.value())?,
        RuntimeValue::Scalar(value) => serde_json::to_value(value.display_value())
            .map_err(|_| ResultQueryApplicationError::UnrepresentableValue)?,
        RuntimeValue::Resource(value) => value.as_ref().into(),
        RuntimeValue::Grouped(groups) => serde_json::json!({
            "kind": "groupedDataFrame",
            "keys": groups.keys(),
            "columns": groups.source().schema().fields().iter().map(|field| field.name()).collect::<Vec<_>>(),
        }),
        RuntimeValue::Relation(_) | RuntimeValue::Series(_) | RuntimeValue::LinearRegression(_) => {
            return Err(ResultQueryApplicationError::UnrepresentableValue);
        }
        RuntimeValue::List(values) => values
            .iter()
            .map(runtime_value_to_json)
            .collect::<Result<Vec<_>, _>>()?
            .into(),
        RuntimeValue::Record(values) => values
            .iter()
            .map(|(key, value)| Ok((key.to_string(), runtime_value_to_json(value)?)))
            .collect::<Result<std::collections::BTreeMap<_, _>, ResultQueryApplicationError>>()?
            .into_iter()
            .collect::<serde_json::Map<_, _>>()
            .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_data_contract::TabularScalar;

    #[test]
    fn inline_wire_budget_counts_escaped_bytes_without_an_encoded_buffer() {
        let encode = |text: String| {
            encode_inline_projection(ResultValueProjection::Value(RuntimeValue::Scalar(
                TabularScalar::String(text.into()),
            )))
        };
        assert!(encode("x".repeat(MAX_INLINE_RESULT_JSON_BYTES - 2)).is_ok());
        assert!(matches!(
            encode("x".repeat(MAX_INLINE_RESULT_JSON_BYTES)),
            Err(ResultQueryApplicationError::PageTooLarge)
        ));
        assert!(matches!(
            encode("\n".repeat(MAX_INLINE_RESULT_JSON_BYTES / 2)),
            Err(ResultQueryApplicationError::PageTooLarge)
        ));
    }
}
