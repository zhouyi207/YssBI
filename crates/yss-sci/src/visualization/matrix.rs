use super::*;
use statrs::distribution::{ChiSquared, ContinuousCDF};
use yss_sci_contract::association::CorrelationOptions;
use yss_sci_contract::visualization::*;

pub fn correlation(
    labels: &[String],
    columns: &[Vec<f64>],
    control: &ScientificExecutionControl,
) -> Result<CorrelationPlot> {
    if labels.len() != columns.len() || !(2..=MAX_PLOT_GROUPS).contains(&columns.len()) {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    let observations = aligned(
        &columns.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        control,
    )?;
    if observations < 3 {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    let varying = columns
        .iter()
        .map(|values| values.iter().any(|value| *value != values[0]))
        .collect::<Vec<_>>();
    let mut matrix = vec![vec![None; columns.len()]; columns.len()];
    let mut p_matrix = matrix.clone();
    for i in 0..columns.len() {
        control.check()?;
        if varying[i] {
            matrix[i][i] = Some(1.0);
        }
        for j in 0..i {
            if !varying[i] || !varying[j] {
                continue;
            }
            let result = crate::association::pearson(
                &columns[i],
                &columns[j],
                CorrelationOptions::default(),
                control,
            )?;
            matrix[i][j] = Some(result.coefficient);
            matrix[j][i] = matrix[i][j];
            p_matrix[i][j] = result.inference.p_value;
            p_matrix[j][i] = p_matrix[i][j];
        }
    }
    Ok(CorrelationPlot {
        labels: labels.to_vec(),
        matrix,
        p_matrix,
        observations,
    })
}

pub fn correlogram(
    values: &[f64],
    maximum_lag: usize,
    control: &ScientificExecutionControl,
) -> Result<CorrelogramPlot> {
    validate(values, control)?;
    if values.len() < 4
        || maximum_lag == 0
        || maximum_lag > 40
        || values.iter().all(|value| *value == values[0])
    {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let lag = maximum_lag.min(values.len() / 2 - 1);
    let result = crate::time_series::acf_pacf::compute_acf_pacf(values, lag, control)?;
    let mut sum = 0.0;
    let n = values.len() as f64;
    let mut acf = Vec::new();
    for (lag, value) in result.acf.iter().copied().enumerate().skip(1) {
        control.check()?;
        sum += value * value / (n - lag as f64);
        let q_stat = finite(n * (n + 2.0) * sum)?;
        let chi = ChiSquared::new(lag as f64)
            .map_err(|_| ScientificComputationError::ComputationFailed)?;
        acf.push(CorrelogramPoint {
            lag,
            value,
            q_stat: Some(q_stat),
            p_value: Some(finite(chi.sf(q_stat))?),
        });
    }
    let pacf = result
        .pacf
        .iter()
        .copied()
        .enumerate()
        .map(|(i, value)| CorrelogramPoint {
            lag: i + 1,
            value,
            q_stat: None,
            p_value: None,
        })
        .collect();
    Ok(CorrelogramPlot {
        acf,
        pacf,
        ci_half_width: result.ci_half_width,
        n: result.n,
    })
}

pub fn heatmap(
    labels: &[String],
    columns: &[Vec<f64>],
    control: &ScientificExecutionControl,
) -> Result<HeatmapPlot> {
    if labels.len() != columns.len() || columns.is_empty() || columns.len() > MAX_PLOT_GROUPS {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    let length = aligned(
        &columns.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        control,
    )?;
    let indices = sample_indices(length, MAX_HEATMAP_ROWS);
    let matrix = indices
        .iter()
        .map(|row| columns.iter().map(|column| column[*row]).collect())
        .collect();
    let y_labels = indices
        .iter()
        .map(|index| (index + 1).to_string())
        .collect();
    Ok(HeatmapPlot {
        x_labels: labels.to_vec(),
        y_labels,
        matrix,
        metadata: metadata(length, indices.len()),
    })
}
