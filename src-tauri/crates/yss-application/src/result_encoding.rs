//! Shared result protocol mapping for desktop IPC and Harness adapters.
use crate::graph::results::report::LinearRegressionReportProjection;
use crate::graph::results::{ResultQueryApplicationError, ResultValueProjection};
use yss_graph_execution::result::ResultReference;
use yss_node_kernel::RuntimeValue;

pub(crate) const MAX_INLINE_RESULT_JSON_BYTES: usize = 64 * 1024;

pub(crate) fn report_to_json(projection: LinearRegressionReportProjection) -> serde_json::Value {
    serde_json::json!({
        "title": projection.title,
        "endog_name": projection.endog_name,
        "paramNames": projection.param_names,
        "resultRef": {
            "executionSessionId": projection.reference.execution_session_id.as_uuid().to_string(),
            "resultId": projection.reference.result_id.get().to_string(),
        },
        "model_basic_info": projection.model,
        "diagnostic_info": { "cond_no": projection.condition_number },
        "coefficients": { "kind": "tableRef", "part": "coefficients", "rowCount": projection.coefficient_count },
        "observations": { "kind": "tableRef", "part": "observations", "rowCount": projection.observation_count },
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
        ResultValueProjection::LinearReport(report) => report_to_json(*report),
    };
    // Count encoded bytes without allocating a second, potentially large byte buffer.
    struct Budget(usize);
    impl std::io::Write for Budget {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_sub(bytes.len())
                .ok_or_else(|| std::io::Error::other("inline result budget exceeded"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Budget(MAX_INLINE_RESULT_JSON_BYTES), &value)
        .map_err(|_| ResultQueryApplicationError::PageTooLarge)?;
    Ok(value)
}

pub(crate) fn runtime_value_to_json(
    value: &RuntimeValue,
) -> Result<serde_json::Value, ResultQueryApplicationError> {
    Ok(match value {
        RuntimeValue::Annotated(value) => runtime_value_to_json(value.value())?,
        RuntimeValue::Scalar(value) => serde_json::to_value(value.display_value())
            .map_err(|_| ResultQueryApplicationError::UnrepresentableValue)?,
        RuntimeValue::Resource(value) => value.as_ref().into(),
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
