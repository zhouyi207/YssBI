use super::{common::*, cox::baseline_at, nonparametric::risk_at};
use yss_sci_contract::{survival::*, visualization::*};

fn predictions(
    time: &[f64],
    event: &[f64],
    predicted: &[f64],
    horizon: f64,
    control: &Control,
) -> Result<()> {
    data(time, event, &[], control)?;
    positive_horizon(horizon)?;
    if predicted.len() != time.len() {
        return Err(invalid(Violation::ShapeMismatch));
    }
    for (i, value) in predicted.iter().enumerate() {
        if i % 1024 == 0 {
            control.check()?;
        }
        if !value.is_finite() {
            return Err(invalid(Violation::NonFiniteInput));
        }
        if !(0.0..=1.0).contains(value) {
            return Err(invalid(Violation::DataOutOfRange));
        }
    }
    Ok(())
}
pub fn calibration(
    time: &[f64],
    event: &[f64],
    predicted: &[f64],
    horizon: f64,
    requested_bins: usize,
    control: &Control,
) -> Result<CalibrationPlot> {
    predictions(time, event, predicted, horizon, control)?;
    if requested_bins < 2 {
        return Err(parameter());
    }
    let mut order = (0..time.len()).collect::<Vec<_>>();
    order.sort_by(|&a, &b| predicted[a].total_cmp(&predicted[b]));
    let width = time.len().div_ceil(requested_bins);
    let mut bins = Vec::new();
    let mut begin = 0;
    while begin < order.len() {
        control.check()?;
        let mut end = (begin + width).min(order.len());
        // Equal predictions always share a bin; fewer bins than requested are possible.
        while end < order.len() && predicted[order[end]] == predicted[order[end - 1]] {
            end += 1;
        }
        let indices = &order[begin..end];
        let (risk, confidence_interval) = risk_at(time, event, indices, horizon, control)?;
        bins.push(CalibrationBin {
            observations: indices.len(),
            predicted_range: [predicted[order[begin]], predicted[order[end - 1]]],
            mean_prediction: indices
                .iter()
                .map(|&i| predicted[i] / indices.len() as f64)
                .sum(),
            observed_risk: risk,
            confidence_interval,
        });
        begin = end;
    }
    Ok(CalibrationPlot {
        horizon,
        plot: XyPlot {
            data: bins
                .iter()
                .map(|b| PlotPoint {
                    x: b.mean_prediction,
                    y: b.observed_risk,
                })
                .collect(),
            x_label: format!("Predicted event probability at {horizon}"),
            y_label: "Kaplan-Meier observed event probability".into(),
            reference_lines: vec![ReferenceLine {
                start: PlotPoint { x: 0.0, y: 0.0 },
                end: PlotPoint { x: 1.0, y: 1.0 },
            }],
            metadata: PlotMetadata {
                observations: time.len(),
                displayed: bins.len(),
                sampled: false,
            },
        },
        bins,
    })
}

pub fn decision_curve(
    time: &[f64],
    event: &[f64],
    predicted: &[f64],
    options: DecisionOptions,
    control: &Control,
) -> Result<DecisionPlot> {
    predictions(time, event, predicted, options.horizon, control)?;
    let (lo, hi) = (options.minimum_threshold, options.maximum_threshold);
    if !lo.is_finite()
        || !hi.is_finite()
        || lo <= 0.0
        || hi >= 1.0
        || lo >= hi
        || options.points < 2
    {
        return Err(parameter());
    }
    let all = (0..time.len()).collect::<Vec<_>>();
    let overall = risk_at(time, event, &all, options.horizon, control)?.0;
    let mut estimates = Vec::with_capacity(options.points);
    for i in 0..options.points {
        control.check()?;
        let threshold = lo + (hi - lo) * i as f64 / (options.points - 1) as f64;
        let selected = all
            .iter()
            .copied()
            .filter(|&j| predicted[j] >= threshold)
            .collect::<Vec<_>>();
        let odds = threshold / (1.0 - threshold);
        let model = if selected.is_empty() {
            0.0
        } else {
            let risk = risk_at(time, event, &selected, options.horizon, control)?.0;
            selected.len() as f64 / time.len() as f64 * (risk - (1.0 - risk) * odds)
        };
        estimates.push(DecisionPoint {
            threshold,
            selected: selected.len(),
            model: finite(model)?,
            treat_all: finite(overall - (1.0 - overall) * odds)?,
            treat_none: 0.0,
        });
    }
    let mut reference_lines = estimates
        .windows(2)
        .map(|w| ReferenceLine {
            start: PlotPoint {
                x: w[0].threshold,
                y: w[0].treat_all,
            },
            end: PlotPoint {
                x: w[1].threshold,
                y: w[1].treat_all,
            },
        })
        .collect::<Vec<_>>();
    reference_lines.push(ReferenceLine {
        start: PlotPoint { x: lo, y: 0.0 },
        end: PlotPoint { x: hi, y: 0.0 },
    });
    Ok(DecisionPlot {
        horizon: options.horizon,
        plot: XyPlot {
            data: estimates
                .iter()
                .map(|p| PlotPoint {
                    x: p.threshold,
                    y: p.model,
                })
                .collect(),
            x_label: "Threshold probability (dashed: treat all / treat none)".into(),
            y_label: "Net benefit".into(),
            reference_lines,
            metadata: PlotMetadata {
                observations: time.len(),
                displayed: estimates.len(),
                sampled: false,
            },
        },
        estimates,
    })
}

fn tick_label(value: f64) -> String {
    if value != 0.0 && (value.abs() < 0.001 || value.abs() >= 1e6) {
        format!("{value:.3e}")
    } else {
        format!("{value:.4}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    }
}
pub fn nomogram(
    model: &CoxResult,
    horizon: f64,
    ticks: usize,
    control: &Control,
) -> Result<NomogramPlot> {
    control.check()?;
    let hazard = baseline_at(model, horizon)?;
    let p = model.coefficients.len();
    if ticks < 2 {
        return Err(parameter());
    }
    if p == 0 || model.predictor_ranges.len() != p || model.predictor_means.len() != p {
        return Err(invalid(Violation::ShapeMismatch));
    }
    if !hazard.is_finite() {
        return Err(invalid(Violation::NonFiniteInput));
    }
    if hazard <= 0.0 {
        return Err(parameter());
    }
    let mut spans = Vec::with_capacity(p);
    let mut lower = Vec::with_capacity(p);
    for j in 0..p {
        let beta = model.coefficients[j].estimate;
        let [lo, hi] = model.predictor_ranges[j];
        let center = model.predictor_means[j];
        if !lo.is_finite() || !hi.is_finite() || !center.is_finite() || !beta.is_finite() {
            return Err(invalid(Violation::NonFiniteInput));
        }
        if lo > hi {
            return Err(invalid(Violation::DataOutOfRange));
        }
        spans.push(finite(beta.abs() * (hi - lo))?);
        lower.push(finite((beta * (lo - center)).min(beta * (hi - center)))?);
    }
    let maximum = spans.iter().copied().fold(0.0, f64::max);
    if maximum <= 0.0 {
        return Err(invalid(Violation::DataOutOfRange));
    }
    let points_per_log_hazard = finite(100.0 / maximum)?;
    let maximum_total_points = finite(spans.iter().sum::<f64>() * points_per_log_hazard)?;
    let mut axes = vec![NomogramAxis {
        label: "Points".into(),
        ticks: (0..ticks)
            .map(|i| {
                let position = i as f64 / (ticks - 1) as f64;
                NomogramTick {
                    position,
                    label: tick_label(position * 100.0),
                }
            })
            .collect(),
    }];
    for (j, &minimum_contribution) in lower.iter().enumerate() {
        control.check()?;
        let beta = model.coefficients[j].estimate;
        let [lo, hi] = model.predictor_ranges[j];
        let mut axis_ticks = Vec::new();
        for i in 0..ticks {
            let v = lo + (hi - lo) * i as f64 / (ticks - 1) as f64;
            let points = (beta * (v - model.predictor_means[j]) - minimum_contribution)
                * points_per_log_hazard;
            axis_ticks.push(NomogramTick {
                position: (points / 100.0).clamp(0.0, 1.0),
                label: tick_label(v),
            });
            if beta == 0.0 {
                break;
            }
        }
        axes.push(NomogramAxis {
            label: model.coefficients[j].term.clone(),
            ticks: axis_ticks,
        });
    }
    let mut total_ticks = Vec::new();
    let mut survival_ticks = Vec::new();
    let origin = lower.iter().sum::<f64>();
    for i in 0..ticks {
        control.check()?;
        let position = i as f64 / (ticks - 1) as f64;
        let total = maximum_total_points * position;
        let survival =
            finite((-(hazard.ln() + origin + total / points_per_log_hazard).exp()).exp())?;
        total_ticks.push(NomogramTick {
            position,
            label: tick_label(total),
        });
        survival_ticks.push(NomogramTick {
            position,
            label: tick_label(survival),
        });
    }
    axes.push(NomogramAxis {
        label: "Total points".into(),
        ticks: total_ticks,
    });
    axes.push(NomogramAxis {
        label: format!("Survival probability at {horizon}"),
        ticks: survival_ticks,
    });
    Ok(NomogramPlot {
        axes,
        horizon,
        maximum_total_points,
        points_per_log_hazard,
        baseline_cumulative_hazard: hazard,
    })
}
