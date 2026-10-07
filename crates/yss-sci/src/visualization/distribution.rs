use super::*;
use crate::descriptive::quantile_sorted as quantile;
use yss_sci_contract::density::KernelDensityInput;
use yss_sci_contract::visualization::*;

fn sorted(values: &[f64], control: &ScientificExecutionControl) -> Result<Vec<f64>> {
    validate(values, control)?;
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    control.check()?;
    Ok(sorted)
}

pub fn ecdf(values: &[f64], control: &ScientificExecutionControl) -> Result<XyPlot> {
    let sorted = sorted(values, control)?;
    let mut points = Vec::new();
    for (index, value) in sorted.iter().enumerate() {
        if index.is_multiple_of(1024) {
            control.check()?;
        }
        if let Some(last) = points
            .last_mut()
            .filter(|last: &&mut PlotPoint| last.x == *value)
        {
            last.y = (index + 1) as f64 / sorted.len() as f64;
        } else {
            points.push(PlotPoint {
                x: *value,
                y: (index + 1) as f64 / sorted.len() as f64,
            });
        }
    }
    let count = points.len();
    let data = sample_indices(count, MAX_PLOT_POINTS)
        .into_iter()
        .map(|i| points[i])
        .collect();
    Ok(XyPlot {
        data,
        x_label: "Values".into(),
        y_label: "Cumulative probability".into(),
        reference_lines: vec![],
        metadata: PlotMetadata {
            observations: values.len(),
            displayed: count.min(MAX_PLOT_POINTS),
            sampled: count > MAX_PLOT_POINTS,
        },
    })
}

pub fn histogram(
    values: &[f64],
    bins: usize,
    control: &ScientificExecutionControl,
) -> Result<HistogramPlot> {
    validate(values, control)?;
    if bins > MAX_PLOT_BINS {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let bins = if bins == 0 {
        ((values.len() as f64).log2().ceil() as usize + 1).min(MAX_PLOT_BINS)
    } else {
        bins
    };
    let mut lower = values.iter().copied().fold(f64::INFINITY, f64::min);
    let mut upper = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if lower == upper {
        let padding = (lower.abs() * 0.05).max(0.5);
        lower = if (lower - padding).is_finite() {
            lower - padding
        } else {
            lower
        };
        upper = if (upper + padding).is_finite() {
            upper + padding
        } else {
            upper
        };
    }
    let scale = lower.abs().max(upper.abs()).max(1.0);
    let range = upper / scale - lower / scale;
    if range <= 0.0 || !range.is_finite() {
        return Err(ScientificComputationError::ComputationFailed);
    }
    let edge = |i: usize| {
        let fraction = i as f64 / bins as f64;
        lower * (1.0 - fraction) + upper * fraction
    };
    let mut data = (0..bins)
        .map(|i| HistogramBin {
            label: format!("{}–{}", edge(i), edge(i + 1)),
            lower: edge(i),
            upper: edge(i + 1),
            count: 0,
        })
        .collect::<Vec<_>>();
    for (index, value) in values.iter().enumerate() {
        if index.is_multiple_of(1024) {
            control.check()?;
        }
        let bin = (((value / scale - lower / scale) / range * bins as f64).floor() as usize)
            .min(bins - 1);
        data[bin].count += 1;
    }
    Ok(HistogramPlot {
        data,
        x_label: "Values".into(),
        y_label: "Frequency".into(),
        observations: values.len(),
    })
}

pub fn kde(
    values: &[f64],
    grid_points: usize,
    control: &ScientificExecutionControl,
) -> Result<XyPlot> {
    validate(values, control)?;
    if values.len() < 2 || !(16..=512).contains(&grid_points) {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let output = crate::density::compute_kernel_density_controlled(
        KernelDensityInput {
            values,
            grid_points,
            min_x: None,
        },
        control,
    )?;
    let data = output
        .points
        .into_iter()
        .map(|point| PlotPoint {
            x: point.x,
            y: point.density,
        })
        .collect();
    Ok(XyPlot {
        data,
        x_label: "Values".into(),
        y_label: "Density".into(),
        reference_lines: vec![],
        metadata: PlotMetadata {
            observations: values.len(),
            displayed: grid_points,
            sampled: false,
        },
    })
}

pub fn box_violin(
    labels: &[String],
    groups: &[Vec<f64>],
    violin: bool,
    control: &ScientificExecutionControl,
) -> Result<DistributionPlot> {
    if labels.len() != groups.len() || groups.is_empty() || groups.len() > MAX_PLOT_GROUPS {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    let mut output = Vec::with_capacity(groups.len());
    for (label, values) in labels.iter().zip(groups) {
        let sorted = sorted(values, control)?;
        let q1 = quantile(&sorted, 0.25);
        let median = quantile(&sorted, 0.5);
        let q3 = quantile(&sorted, 0.75);
        let iqr = finite(q3 - q1)?;
        let low_fence = q1 - 1.5 * iqr;
        let high_fence = q3 + 1.5 * iqr;
        let first = sorted.partition_point(|value| *value < low_fence);
        let end = sorted.partition_point(|value| *value <= high_fence);
        let outliers = sorted[..first]
            .iter()
            .chain(&sorted[end..])
            .copied()
            .collect::<Vec<_>>();
        let outlier_count = outliers.len();
        let outliers = sample_indices(outliers.len(), MAX_PLOT_POINTS)
            .into_iter()
            .map(|i| outliers[i])
            .collect();
        let density = if violin {
            kde(values, 128, control)?.data
        } else {
            vec![]
        };
        output.push(DistributionGroup {
            label: label.clone(),
            observations: values.len(),
            lower_whisker: sorted[first],
            q1,
            median,
            q3,
            upper_whisker: sorted[end - 1],
            outliers,
            outlier_count,
            density,
        });
    }
    control.check()?;
    Ok(DistributionPlot { groups: output })
}
