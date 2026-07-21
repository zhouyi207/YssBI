use std::fs::{self, File};
use std::path::Path;

use polars::prelude::{Column, DataFrame, IpcReader, IpcWriter, SerReader, SerWriter};
use serde_json::json;

use crate::julia::worker::{JuliaWorkerManager, JuliaWorkerTask};
use crate::sci::api::time_series::serial_tests::{
    DurbinWatsonResult, SerialTestWithLag, SerialTestsInput, SerialTestsOutput,
};
use crate::sci::engine::SciContext;
use crate::sci::error::SciError;

pub fn compute(
    context: &SciContext<'_>,
    input: SerialTestsInput,
    lags: usize,
) -> Result<SerialTestsOutput, SciError> {
    let julia = context
        .julia
        .ok_or_else(|| SciError::julia_unavailable("SciContext missing Julia context"))?;

    compute_with_worker(julia.app_data_dir, julia.worker, input, lags)
}

fn compute_with_worker(
    app_data_dir: &Path,
    worker: &JuliaWorkerManager,
    input: SerialTestsInput,
    lags: usize,
) -> Result<SerialTestsOutput, SciError> {
    let exog_columns = input
        .exog
        .as_ref()
        .map(|exog| exog_column_names(exog).collect::<Vec<_>>())
        .unwrap_or_default();
    let residuals = input.residuals;
    let exog = input.exog;
    let bg_nomiss0 = input.bg_nomiss0;

    let output = worker
        .run_task(
            app_data_dir,
            JuliaWorkerTask {
                task_id: None,
                operation: "serial_tests".to_string(),
                parameters: json!({
                    "residualColumn": "residual",
                    "lags": lags,
                    "exogColumns": exog_columns,
                    "bgNomiss0": bg_nomiss0,
                }),
            },
            |input_path| write_input(input_path, &residuals, exog.as_deref()),
            None,
        )
        .map_err(SciError::julia_task_failed)?;

    let result = read_output(&output.output_path);
    if let Some(task_dir) = output.output_path.parent() {
        let _ = fs::remove_dir_all(task_dir);
    }
    result
}

fn exog_column_names(exog: &[Vec<f64>]) -> impl Iterator<Item = String> + '_ {
    let column_count = exog.first().map_or(0, Vec::len);
    (0..column_count).map(|index| format!("exog_{index}"))
}

fn write_input(path: &Path, residuals: &[f64], exog: Option<&[Vec<f64>]>) -> Result<(), String> {
    let mut columns = vec![Column::new("residual".into(), residuals)];

    if let Some(exog) = exog {
        let row_count = residuals.len();
        if exog.len() != row_count {
            return Err(format!(
                "exog row count {} does not match residual count {}",
                exog.len(),
                row_count
            ));
        }
        let column_count = exog.first().map_or(0, Vec::len);
        if exog.iter().any(|row| row.len() != column_count) {
            return Err("exog must be a rectangular matrix".to_string());
        }
        for column_index in 0..column_count {
            let values = exog.iter().map(|row| row[column_index]).collect::<Vec<_>>();
            columns.push(Column::new(
                format!("exog_{column_index}").into(),
                values.as_slice(),
            ));
        }
    }

    let mut dataframe = DataFrame::new(residuals.len(), columns)
        .map_err(|error| format!("Failed to create Julia serial-tests input table: {error}"))?;
    let mut file = File::create(path)
        .map_err(|error| format!("Failed to create Julia serial-tests input file: {error}"))?;
    IpcWriter::new(&mut file)
        .finish(&mut dataframe)
        .map_err(|error| format!("Failed to write Julia serial-tests input table: {error}"))
}

fn read_output(path: &Path) -> Result<SerialTestsOutput, SciError> {
    let file = File::open(path).map_err(|error| {
        SciError::julia_task_failed(format!("Julia worker did not write output: {error}"))
    })?;
    let dataframe = IpcReader::new(file).finish().map_err(|error| {
        SciError::julia_task_failed(format!(
            "Failed to read Julia serial-tests output table: {error}"
        ))
    })?;

    Ok(SerialTestsOutput {
        bg: optional_lagged_result(&dataframe, "bg")?,
        q: optional_lagged_result(&dataframe, "q")?,
        dw: DurbinWatsonResult {
            d: required_f64(&dataframe, "dw_d")?,
        },
    })
}

fn optional_lagged_result(
    dataframe: &DataFrame,
    prefix: &str,
) -> Result<Option<SerialTestWithLag>, SciError> {
    let Some(stat) = optional_f64(dataframe, &format!("{prefix}_stat"))? else {
        return Ok(None);
    };
    let p_value = required_f64(dataframe, &format!("{prefix}_p_value"))?;
    let lags = required_i64(dataframe, &format!("{prefix}_lags"))?;
    let lags = usize::try_from(lags).map_err(|_| {
        SciError::julia_task_failed(format!("Julia worker returned invalid {prefix} lags"))
    })?;

    Ok(Some(SerialTestWithLag {
        stat,
        p_value,
        lags,
    }))
}

fn required_f64(dataframe: &DataFrame, column: &str) -> Result<f64, SciError> {
    optional_f64(dataframe, column)?.ok_or_else(|| {
        SciError::julia_task_failed(format!("Julia worker returned null `{column}`"))
    })
}

fn optional_f64(dataframe: &DataFrame, column: &str) -> Result<Option<f64>, SciError> {
    dataframe
        .column(column)
        .map_err(|error| SciError::julia_task_failed(error.to_string()))?
        .f64()
        .map_err(|error| SciError::julia_task_failed(error.to_string()))?
        .get(0)
        .map_or(Ok(None), |value| Ok(Some(value)))
}

fn required_i64(dataframe: &DataFrame, column: &str) -> Result<i64, SciError> {
    dataframe
        .column(column)
        .map_err(|error| SciError::julia_task_failed(error.to_string()))?
        .i64()
        .map_err(|error| SciError::julia_task_failed(error.to_string()))?
        .get(0)
        .ok_or_else(|| {
            SciError::julia_task_failed(format!("Julia worker returned null `{column}`"))
        })
}
