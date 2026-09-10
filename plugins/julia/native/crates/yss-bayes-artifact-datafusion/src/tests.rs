use super::*;
use arrow::array::{ArrayRef, Float64Array, Int64Array, StringArray};

struct Artifact(std::path::PathBuf);
impl Artifact {
    fn new(batch: RecordBatch) -> Self {
        let path = std::env::temp_dir().join(format!(
            "yss-bayes [artifact]-{}.arrow",
            uuid::Uuid::new_v4()
        ));
        let batches = (0..batch.num_rows())
            .step_by(2)
            .map(|offset| Ok(batch.slice(offset, 2.min(batch.num_rows() - offset))));
        yss_tabular_io::write_ipc_batches(&path, &batch.schema(), batches).unwrap();
        Self(path)
    }
}
impl Drop for Artifact {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn reader() -> DataFusionBayesArtifactReader {
    DataFusionBayesArtifactReader::new().unwrap()
}

#[test]
fn posterior_predictive_page_paginates_rows() {
    let artifact = Artifact::new(predictive_batch());
    let page = reader()
        .posterior_predictive_page(&artifact.0, 1, 1)
        .unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.response_transform, "ln");
    assert_eq!(page.rows[0].observation, 2);
    assert_eq!(page.rows[0].model.observed, 2.0);
    assert_eq!(page.rows[0].original.mean, 5.1);
    assert!(
        reader()
            .posterior_predictive_page(&artifact.0, 3, 1)
            .unwrap()
            .rows
            .is_empty()
    );
}

#[test]
fn trace_density_and_autocorrelation_are_projected_from_samples() {
    let artifact = Artifact::new(sample_batch());
    let reader = reader();
    let trace = reader.trace_plot_data(&artifact.0, Some("a"), 1).unwrap();
    assert_eq!(trace.series.len(), 2);
    assert_eq!(trace.series[0].parameter, "a");
    assert_eq!(trace.series[0].points.len(), 1);
    assert_eq!(trace.series[0].points[0].draw, 1);
    let density = reader.density_plot_data(&artifact.0, Some("b"), 8).unwrap();
    assert_eq!(density.grid_points, 8);
    assert_eq!(density.series.len(), 3);
    assert_eq!(density.series[0].parameter, "b");
    assert_eq!(density.series[0].chain, None);
    assert_eq!(density.series[1].chain, Some(1));
    assert_eq!(density.series[2].chain, Some(2));
    assert_eq!(density.series[0].points.len(), 8);
    assert!(
        density.series[0]
            .points
            .iter()
            .all(|point| point.density.is_finite() && point.density >= 0.0)
    );
    let autocorrelation = reader
        .autocorrelation_plot_data(&artifact.0, Some("a"), 2)
        .unwrap();
    assert_eq!(autocorrelation.max_lag, 2);
    assert_eq!(autocorrelation.series.len(), 1);
    assert_eq!(autocorrelation.series[0].points[0].lag, 0);
    assert!((autocorrelation.series[0].points[0].autocorrelation - 1.0).abs() < 1e-12);
}

#[test]
fn posterior_sample_page_filters_and_paginates() {
    let artifact = Artifact::new(sample_batch());
    let reader = reader();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let page = runtime
        .block_on(async { reader.sample_page(&artifact.0, 1, 2, Some("a")) })
        .unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.rows[0].parameter, "a");
    assert_eq!(page.rows[0].chain, 1);
    assert_eq!(page.rows[0].draw, 2);
    assert_eq!(page.rows[0].value, 1.1);
    assert_eq!(page.rows[1].chain, 2);
    assert_eq!(
        reader
            .sample_page(&artifact.0, 0, 100, Some("a' OR true"))
            .unwrap()
            .total,
        0
    );
    let csv = Artifact(artifact.0.with_extension("csv"));
    reader.export_csv(&artifact.0, &csv.0).unwrap();
    let text = std::fs::read_to_string(&csv.0).unwrap();
    assert_eq!(
        text.lines()
            .filter(|line| *line == "parameter,chain,draw,value")
            .count(),
        1
    );
    assert_eq!(text.lines().count(), 6);
    let batches = yss_tabular_io::read_csv_batches(&csv.0, b',', true, 20, 2).unwrap();
    assert_eq!(
        batches
            .map(|batch| batch.unwrap().num_rows())
            .sum::<usize>(),
        5
    );
}

#[test]
fn malformed_sample_rows_and_zero_plot_budgets_fail_closed() {
    let negative = replace(
        sample_batch(),
        "chain",
        Arc::new(Int64Array::from(vec![-1, 1, 1, 2, 2])),
    );
    let artifact = Artifact::new(negative);
    assert_eq!(
        reader().sample_page(&artifact.0, 0, 1, None),
        Err(samples_invalid())
    );
    let non_finite = replace(
        sample_batch(),
        "value",
        Arc::new(Float64Array::from(vec![1., 1.1, 2., 1.2, f64::NAN])),
    );
    let artifact = Artifact::new(non_finite);
    assert_eq!(
        reader().sample_page(&artifact.0, 0, 1, Some("a")),
        Err(samples_invalid()),
        "malformed rows outside the page and selected parameter invalidate the artifact"
    );
    let artifact = Artifact::new(sample_batch());
    assert_eq!(
        reader().trace_plot_data(&artifact.0, None, 0),
        Err(samples_invalid())
    );
    assert_eq!(
        reader().density_plot_data(&artifact.0, None, 1),
        Err(samples_invalid())
    );
}

#[test]
fn inconsistent_or_empty_predictive_rows_fail_closed() {
    let inconsistent = replace(
        predictive_batch(),
        "response_transform",
        Arc::new(StringArray::from(vec!["ln", "identity", "ln"])),
    );
    let artifact = Artifact::new(inconsistent);
    assert_eq!(
        reader().posterior_predictive_page(&artifact.0, 0, 3),
        Err(posterior_predictive_invalid())
    );
    let artifact = Artifact::new(predictive_batch().slice(0, 0));
    assert_eq!(
        reader().posterior_predictive_page(&artifact.0, 0, 1),
        Err(posterior_predictive_invalid())
    );
    let non_finite = replace(
        predictive_batch(),
        "q975_original",
        Arc::new(Float64Array::from(vec![3.8, 5.8, f64::INFINITY])),
    );
    let artifact = Artifact::new(non_finite);
    assert_eq!(
        reader().posterior_predictive_page(&artifact.0, 0, 1),
        Err(posterior_predictive_invalid())
    );
}

fn replace(batch: RecordBatch, name: &str, array: ArrayRef) -> RecordBatch {
    RecordBatch::try_from_iter(batch.schema().fields().iter().zip(batch.columns()).map(
        |(field, previous)| {
            (
                field.name().clone(),
                if field.name() == name {
                    array.clone()
                } else {
                    previous.clone()
                },
            )
        },
    ))
    .unwrap()
}
fn sample_batch() -> RecordBatch {
    RecordBatch::try_from_iter(vec![
        (
            "parameter",
            Arc::new(StringArray::from(vec!["a", "a", "b", "a", "b"])) as ArrayRef,
        ),
        ("chain", Arc::new(Int64Array::from(vec![1, 1, 1, 2, 2]))),
        ("draw", Arc::new(Int64Array::from(vec![1, 2, 1, 1, 2]))),
        (
            "value",
            Arc::new(Float64Array::from(vec![1., 1.1, 2., 1.2, 2.1])),
        ),
    ])
    .unwrap()
}
fn predictive_batch() -> RecordBatch {
    let mut columns = vec![
        (
            "observation",
            Arc::new(Int64Array::from(vec![1, 2, 3])) as ArrayRef,
        ),
        (
            "response_transform",
            Arc::new(StringArray::from(vec!["ln", "ln", "ln"])) as ArrayRef,
        ),
    ];
    for (name, values) in [
        ("observed_model", vec![1., 2., 3.]),
        ("mean_model", vec![1.1, 2.1, 3.1]),
        ("q025_model", vec![0.5, 1.4, 2.2]),
        ("q975_model", vec![1.8, 2.8, 3.7]),
        ("observed_original", vec![3., 5., 7.]),
        ("mean_original", vec![3.1, 5.1, 6.9]),
        ("q025_original", vec![2.5, 4.4, 6.2]),
        ("q975_original", vec![3.8, 5.8, 7.7]),
    ] {
        columns.push((name, Arc::new(Float64Array::from(values))));
    }
    RecordBatch::try_from_iter(columns).unwrap()
}
