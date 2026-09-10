use super::*;
use std::collections::BTreeMap;

pub(crate) fn trace_plot_data(
    rows: Vec<PosteriorSampleRow>,
    max_points_per_chain: usize,
) -> Result<TracePlotData, BayesArtifactReadError> {
    if max_points_per_chain == 0 {
        return Err(samples_invalid());
    }
    let mut grouped: BTreeMap<(String, usize), Vec<TracePoint>> = BTreeMap::new();
    for row in rows {
        grouped
            .entry((row.parameter, row.chain))
            .or_default()
            .push(TracePoint {
                draw: row.draw,
                value: row.value,
            });
    }

    let mut stride = 1;
    let series = grouped
        .into_iter()
        .map(|((parameter, chain), mut points)| {
            points.sort_by_key(|point| point.draw);
            let local_stride = points.len().div_ceil(max_points_per_chain).max(1);
            stride = stride.max(local_stride);
            let points = points
                .into_iter()
                .enumerate()
                .filter_map(|(index, point)| (index % local_stride == 0).then_some(point))
                .collect();
            TraceSeries {
                parameter,
                chain,
                points,
            }
        })
        .collect();

    Ok(TracePlotData {
        series,
        max_points_per_chain,
        stride,
    })
}

pub(crate) fn density_plot_data(
    rows: Vec<PosteriorSampleRow>,
    grid_points: usize,
) -> Result<DensityPlotData, BayesArtifactReadError> {
    if grid_points < 2 {
        return Err(samples_invalid());
    }
    let mut grouped: BTreeMap<String, BTreeMap<usize, Vec<f64>>> = BTreeMap::new();
    for row in rows {
        grouped
            .entry(row.parameter)
            .or_default()
            .entry(row.chain)
            .or_default()
            .push(row.value);
    }

    let mut series = Vec::new();
    for (parameter, chains) in grouped {
        let pooled = chains.values().flatten().copied().collect::<Vec<_>>();
        series.push(density_series(&parameter, None, &pooled, grid_points));
        series.extend(
            chains.into_iter().map(|(chain, values)| {
                density_series(&parameter, Some(chain), &values, grid_points)
            }),
        );
    }

    Ok(DensityPlotData {
        series,
        grid_points,
    })
}

fn density_series(
    parameter: &str,
    chain: Option<usize>,
    values: &[f64],
    grid_points: usize,
) -> DensitySeries {
    DensitySeries {
        parameter: parameter.to_string(),
        chain,
        points: density_points(values, grid_points),
    }
}

fn density_points(values: &[f64], grid_points: usize) -> Vec<DensityPoint> {
    if values.is_empty() || grid_points < 2 {
        return Vec::new();
    }
    // Sample rows are already validated. Preserve the plugin's Gaussian/Silverman
    // projection without linking the host's full scientific runtime.
    let count = values.len() as f64;
    let mean = values.iter().sum::<f64>() / count;
    let sigma = (values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (count - 1.0))
        .sqrt();
    let bandwidth = if sigma.is_finite() && sigma > 0.0 {
        1.06 * sigma * count.powf(-0.2)
    } else {
        1.0
    };
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let padding = ((max - min) * 0.15).max(bandwidth * 2.0).max(0.1);
    let lower = min - padding;
    let upper = max + padding;
    (0..grid_points)
        .map(|index| {
            let x = lower + index as f64 / (grid_points - 1) as f64 * (upper - lower);
            let density = values
                .iter()
                .map(|value| {
                    let standardized = (x - value) / bandwidth;
                    0.398_942_280_401_432_7 * (-0.5 * standardized * standardized).exp()
                })
                .sum::<f64>()
                / (count * bandwidth);
            DensityPoint { x, density }
        })
        .collect()
}

#[cfg(test)]
#[test]
fn density_grid_preserves_bandwidth_and_constant_chain_fallback() {
    let points = density_points(&[0.0, 1.0, 2.0], 3);
    assert!((points[0].x - -1.701_812_110_931_689_3).abs() < 1e-12);
    assert!((points[1].x - 1.0).abs() < 1e-12);
    assert!((points[1].density - 0.312_966_247_632_955_2).abs() < 1e-12);
    let constant = density_points(&[4.0, 4.0], 5);
    assert_eq!((constant[0].x, constant[4].x), (2.0, 6.0));
    assert!((constant[2].density - 0.398_942_280_401_432_7).abs() < 1e-15);
}

pub(crate) fn autocorrelation_plot_data(
    rows: Vec<PosteriorSampleRow>,
    max_lag: usize,
) -> Result<AutocorrelationPlotData, BayesArtifactReadError> {
    let mut grouped: BTreeMap<(String, usize), Vec<PosteriorSampleRow>> = BTreeMap::new();
    for row in rows {
        grouped
            .entry((row.parameter.clone(), row.chain))
            .or_default()
            .push(row);
    }

    let series = grouped
        .into_iter()
        .filter_map(|((parameter, chain), mut rows)| {
            rows.sort_by_key(|row| row.draw);
            let values = rows.into_iter().map(|row| row.value).collect::<Vec<_>>();
            let points = autocorrelation_points(&values, max_lag);
            (!points.is_empty()).then_some(AutocorrelationSeries {
                parameter,
                chain,
                points,
            })
        })
        .collect();

    Ok(AutocorrelationPlotData { series, max_lag })
}

fn autocorrelation_points(values: &[f64], max_lag: usize) -> Vec<AutocorrelationPoint> {
    if values.len() < 2 {
        return Vec::new();
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| {
            let centered = value - mean;
            centered * centered
        })
        .sum::<f64>();
    if variance <= f64::EPSILON {
        return Vec::new();
    }

    let max_lag = max_lag.min(values.len() - 1);
    (0..=max_lag)
        .map(|lag| {
            let covariance = values
                .iter()
                .take(values.len() - lag)
                .zip(values.iter().skip(lag))
                .map(|(left, right)| (left - mean) * (right - mean))
                .sum::<f64>();
            AutocorrelationPoint {
                lag,
                autocorrelation: covariance / variance,
            }
        })
        .collect()
}
