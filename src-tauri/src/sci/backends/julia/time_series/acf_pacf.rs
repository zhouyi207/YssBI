use std::fs;
use std::path::Path;

use crate::julia::worker::{JuliaWorkerManager, JuliaWorkerTask};
use crate::sci::api::time_series::acf_pacf::{AcfPacfInput, AcfPacfOutput};
use crate::sci::engine::SciContext;
use crate::sci::error::SciError;
use crate::tabular::dataframe_io::{read_ipc_dataframe, write_ipc_dataframe};
use polars::prelude::{Column, DataFrame};
use serde_json::json;

pub fn compute(
    context: &SciContext<'_>,
    input: AcfPacfInput,
    max_lag: usize,
) -> Result<AcfPacfOutput, SciError> {
    let julia = context
        .julia
        .ok_or_else(|| SciError::julia_unavailable("SciContext missing Julia context"))?;

    compute_with_worker(julia.app_data_dir, julia.worker, input, max_lag)
}

fn compute_with_worker(
    app_data_dir: &Path,
    worker: &JuliaWorkerManager,
    input: AcfPacfInput,
    max_lag: usize,
) -> Result<AcfPacfOutput, SciError> {
    let n = input.residuals.len();
    let values = input.residuals;
    let output = worker
        .run_task(
            app_data_dir,
            JuliaWorkerTask {
                task_id: None,
                operation: "acf_pacf".to_string(),
                parameters: json!({ "column": "value", "maxLag": max_lag }),
            },
            |input_path| write_input(input_path, &values),
            None,
        )
        .map_err(SciError::julia_task_failed)?;

    let result = read_output(&output.output_path, n);
    if let Some(task_dir) = output.output_path.parent() {
        let _ = fs::remove_dir_all(task_dir);
    }
    result
}

fn write_input(path: &Path, values: &[f64]) -> Result<(), String> {
    let mut dataframe = DataFrame::new(values.len(), vec![Column::new("value".into(), values)])
        .map_err(|error| format!("Failed to create Julia input table: {error}"))?;
    write_ipc_dataframe(path, &mut dataframe)
        .map_err(|error| format!("Failed to write Julia input table: {error}"))
}

fn read_output(path: &Path, n: usize) -> Result<AcfPacfOutput, SciError> {
    let dataframe = read_ipc_dataframe(path).map_err(|error| {
        SciError::julia_task_failed(format!("Failed to read Julia output table: {error}"))
    })?;
    let acf = dataframe
        .column("acf")
        .map_err(|error| SciError::julia_task_failed(error.to_string()))?
        .f64()
        .map_err(|error| SciError::julia_task_failed(error.to_string()))?
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| SciError::julia_task_failed("Julia worker returned null ACF values"))?;
    let pacf = dataframe
        .column("pacf")
        .map_err(|error| SciError::julia_task_failed(error.to_string()))?
        .f64()
        .map_err(|error| SciError::julia_task_failed(error.to_string()))?
        .into_iter()
        .skip(1)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| SciError::julia_task_failed("Julia worker returned null PACF values"))?;

    Ok(AcfPacfOutput { acf, pacf, n })
}
