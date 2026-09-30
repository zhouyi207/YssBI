use super::*;
use statrs::distribution::{ContinuousCDF, Normal, StudentsT};
use yss_sci_contract::visualization::*;

pub fn xy(
    x: &[f64],
    y: &[f64],
    sort_x: bool,
    control: &ScientificExecutionControl,
) -> Result<XyPlot> {
    let length = aligned(&[x, y], control)?;
    let mut indices = (0..length).collect::<Vec<_>>();
    if sort_x {
        indices.sort_by(|a, b| x[*a].total_cmp(&x[*b]));
        control.check()?;
    }
    let data = sample_indices(length, MAX_PLOT_POINTS)
        .into_iter()
        .map(|i| PlotPoint {
            x: x[indices[i]],
            y: y[indices[i]],
        })
        .collect();
    Ok(XyPlot {
        data,
        x_label: "X".into(),
        y_label: "Y".into(),
        reference_lines: vec![],
        metadata: sample_metadata(length),
    })
}

pub fn error_bars(
    x: &[f64],
    y: &[f64],
    lower: &[f64],
    upper: &[f64],
    control: &ScientificExecutionControl,
) -> Result<IntervalPlot> {
    let length = aligned(&[x, y, lower, upper], control)?;
    for i in 0..length {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        if lower[i] > y[i] || y[i] > upper[i] {
            return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
        }
    }
    let data = sample_indices(length, MAX_PLOT_POINTS)
        .into_iter()
        .map(|i| IntervalPoint {
            x: x[i],
            y: y[i],
            lower: lower[i],
            upper: upper[i],
        })
        .collect();
    Ok(IntervalPlot {
        data,
        metadata: sample_metadata(length),
    })
}

pub fn bubble(
    x: &[f64],
    y: &[f64],
    sizes: &[f64],
    control: &ScientificExecutionControl,
) -> Result<BubblePlot> {
    let length = aligned(&[x, y, sizes], control)?;
    if sizes.iter().any(|value| *value < 0.0) || !sizes.iter().any(|value| *value > 0.0) {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let mut indices = sample_indices(length, MAX_PLOT_POINTS);
    let largest = sizes
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, _)| i)
        .unwrap();
    if !indices.contains(&largest) {
        let middle = indices.len() / 2;
        indices[middle] = largest;
        indices.sort_unstable();
    }
    let data = indices
        .into_iter()
        .map(|i| BubblePoint {
            x: x[i],
            y: y[i],
            size: sizes[i],
        })
        .collect();
    Ok(BubblePlot {
        data,
        metadata: sample_metadata(length),
    })
}

pub fn quadrant(
    x: &[f64],
    y: &[f64],
    x_cut: f64,
    y_cut: f64,
    control: &ScientificExecutionControl,
) -> Result<QuadrantPlot> {
    aligned(&[x, y], control)?;
    if !x_cut.is_finite() || !y_cut.is_finite() {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let mut counts = [0usize; 4];
    for (i, (x, y)) in x.iter().zip(y).enumerate() {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        counts[match (*x >= x_cut, *y >= y_cut) {
            (true, true) => 0,
            (false, true) => 1,
            (false, false) => 2,
            (true, false) => 3,
        }] += 1;
    }
    let mut plot = xy(x, y, false, control)?;
    let x_min = x.iter().copied().fold(x_cut, f64::min);
    let x_max = x.iter().copied().fold(x_cut, f64::max);
    let y_min = y.iter().copied().fold(y_cut, f64::min);
    let y_max = y.iter().copied().fold(y_cut, f64::max);
    plot.reference_lines = vec![
        ReferenceLine {
            start: PlotPoint { x: x_cut, y: y_min },
            end: PlotPoint { x: x_cut, y: y_max },
        },
        ReferenceLine {
            start: PlotPoint { x: x_min, y: y_cut },
            end: PlotPoint { x: x_max, y: y_cut },
        },
    ];
    Ok(QuadrantPlot {
        plot,
        x_cut,
        y_cut,
        counts,
    })
}

pub fn probability(
    values: &[f64],
    mode: ProbabilityPlotMode,
    estimate: bool,
    mean: f64,
    standard_deviation: f64,
    control: &ScientificExecutionControl,
) -> Result<ProbabilityPlot> {
    validate(values, control)?;
    if values.len() < 2 {
        return Err(invalid(ScientificInputViolation::EmptyInput));
    }
    let (mean, standard_deviation) = if estimate {
        let scale = values.iter().map(|v| v.abs()).fold(1.0, f64::max);
        let mean = values.iter().map(|v| v / scale).sum::<f64>() / values.len() as f64;
        let variance = values
            .iter()
            .map(|v| (v / scale - mean).powi(2))
            .sum::<f64>()
            / (values.len() - 1) as f64;
        (finite(mean * scale)?, finite(variance.sqrt() * scale)?)
    } else {
        (mean, standard_deviation)
    };
    if !mean.is_finite() || !standard_deviation.is_finite() || standard_deviation <= 0.0 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let normal = Normal::new(mean, standard_deviation)
        .map_err(|_| invalid(ScientificInputViolation::ParameterOutOfRange))?;
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    control.check()?;
    let mut data = Vec::new();
    for i in sample_indices(values.len(), MAX_PLOT_POINTS) {
        control.check()?;
        let p = (i as f64 + 0.5) / values.len() as f64;
        data.push(match mode {
            ProbabilityPlotMode::Pp => PlotPoint {
                x: finite(normal.cdf(values[i]))?,
                y: p,
            },
            ProbabilityPlotMode::Qq => PlotPoint {
                x: finite(normal.inverse_cdf(p))?,
                y: values[i],
            },
        });
    }
    let (lower, upper) = if mode == ProbabilityPlotMode::Pp {
        (0.0, 1.0)
    } else {
        (
            data.iter()
                .map(|p| p.x.min(p.y))
                .fold(f64::INFINITY, f64::min),
            data.iter()
                .map(|p| p.x.max(p.y))
                .fold(f64::NEG_INFINITY, f64::max),
        )
    };
    Ok(ProbabilityPlot {
        plot: XyPlot {
            data,
            x_label: if mode == ProbabilityPlotMode::Pp {
                "Theoretical probability"
            } else {
                "Theoretical quantile"
            }
            .into(),
            y_label: if mode == ProbabilityPlotMode::Pp {
                "Empirical probability"
            } else {
                "Observed quantile"
            }
            .into(),
            reference_lines: vec![ReferenceLine {
                start: PlotPoint { x: lower, y: lower },
                end: PlotPoint { x: upper, y: upper },
            }],
            metadata: sample_metadata(values.len()),
        },
        mode,
        reference_mean: mean,
        reference_standard_deviation: standard_deviation,
    })
}

pub fn roc(
    labels: &[bool],
    scores: &[f64],
    control: &ScientificExecutionControl,
) -> Result<RocPlot> {
    validate(scores, control)?;
    if labels.len() != scores.len() {
        return Err(invalid(ScientificInputViolation::ShapeMismatch));
    }
    let positives = labels.iter().filter(|value| **value).count();
    let negatives = labels.len() - positives;
    if positives == 0 || negatives == 0 {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let mut indices = (0..scores.len()).collect::<Vec<_>>();
    indices.sort_by(|a, b| scores[*b].total_cmp(&scores[*a]));
    control.check()?;
    let mut points = vec![PlotPoint { x: 0.0, y: 0.0 }];
    let (mut true_positives, mut false_positives) = (0usize, 0usize);
    let mut i = 0;
    while i < indices.len() {
        control.check()?;
        let score = scores[indices[i]];
        while i < indices.len() && scores[indices[i]] == score {
            if i.is_multiple_of(1024) {
                control.check()?;
            }
            if labels[indices[i]] {
                true_positives += 1;
            } else {
                false_positives += 1;
            }
            i += 1;
        }
        points.push(PlotPoint {
            x: false_positives as f64 / negatives as f64,
            y: true_positives as f64 / positives as f64,
        });
    }
    let auc = points
        .windows(2)
        .map(|p| (p[1].x - p[0].x) * (p[1].y + p[0].y) * 0.5)
        .sum::<f64>();
    let sampled = points.len() > MAX_PLOT_POINTS;
    let data = sample_indices(points.len(), MAX_PLOT_POINTS)
        .into_iter()
        .map(|i| points[i])
        .collect();
    Ok(RocPlot {
        plot: XyPlot {
            data,
            x_label: "False positive rate".into(),
            y_label: "True positive rate".into(),
            reference_lines: vec![ReferenceLine {
                start: PlotPoint { x: 0.0, y: 0.0 },
                end: PlotPoint { x: 1.0, y: 1.0 },
            }],
            metadata: PlotMetadata {
                observations: labels.len(),
                displayed: points.len().min(MAX_PLOT_POINTS),
                sampled,
            },
        },
        auc: finite(auc)?,
        positives,
        negatives,
    })
}

pub fn coefficients(
    labels: &[String],
    values: &[f64],
    standard_errors: &[f64],
    degrees_of_freedom: f64,
    confidence: f64,
    control: &ScientificExecutionControl,
) -> Result<CoefficientPlot> {
    let length = aligned(&[values, standard_errors], control)?;
    if labels.len() != length
        || length > MAX_PLOT_POINTS
        || !(0.0..1.0).contains(&confidence)
        || confidence == 0.0
        || !degrees_of_freedom.is_finite()
        || degrees_of_freedom <= 0.0
        || standard_errors.iter().any(|v| *v < 0.0)
    {
        return Err(invalid(ScientificInputViolation::ParameterOutOfRange));
    }
    let distribution = StudentsT::new(0.0, 1.0, degrees_of_freedom)
        .map_err(|_| invalid(ScientificInputViolation::ParameterOutOfRange))?;
    let critical = finite(distribution.inverse_cdf((1.0 + confidence) / 2.0))?;
    let data = (0..length)
        .map(|i| {
            Ok(CoefficientPoint {
                label: labels[i].clone(),
                value: values[i],
                lower: finite(values[i] - critical * standard_errors[i])?,
                upper: finite(values[i] + critical * standard_errors[i])?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    control.check()?;
    Ok(CoefficientPlot {
        data,
        confidence_level: confidence,
    })
}
