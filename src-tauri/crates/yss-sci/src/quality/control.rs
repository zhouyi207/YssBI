use super::process::{MR_D4, Process};
use super::*;
use yss_sci_contract::visualization::{PlotMetadata, PlotPoint, ReferenceLine, XyPlot};
pub fn control_chart(
    values: &[f64],
    kind: ControlChartKind,
    control: &Control,
) -> Result<ControlChartResult> {
    let p = Process::new(values, &[], control)?;
    let (center, lower, upper) = match kind {
        ControlChartKind::Individuals => (
            p.mean,
            p.mean - 3. * p.within_sigma(),
            p.mean + 3. * p.within_sigma(),
        ),
        ControlChartKind::MovingRange => (p.mean_moving_range, 0., MR_D4 * p.mean_moving_range),
    };
    let (center, lower, upper) = (
        finite(center * p.scale)?,
        finite(lower * p.scale)?,
        finite(upper * p.scale)?,
    );
    let start = usize::from(kind == ControlChartKind::MovingRange);
    let mut rows = Vec::with_capacity(values.len() - start);
    for (i, &observed) in values.iter().enumerate().skip(start) {
        if i.is_multiple_of(1024) {
            control.check()?;
        }
        let value = if kind == ControlChartKind::Individuals {
            observed
        } else {
            finite((p.values[i] - p.values[i - 1]).abs() * p.scale)?
        };
        rows.push(ControlChartRow {
            observation: i + 1,
            value,
            outside: value < lower || value > upper,
        });
    }
    let reference_lines = [lower, center, upper]
        .into_iter()
        .map(|y| ReferenceLine {
            start: PlotPoint {
                x: (start + 1) as f64,
                y,
            },
            end: PlotPoint {
                x: values.len() as f64,
                y,
            },
        })
        .collect();
    let plot = XyPlot {
        data: rows
            .iter()
            .map(|r| PlotPoint {
                x: r.observation as f64,
                y: r.value,
            })
            .collect(),
        x_label: "Observation order".into(),
        y_label: if kind == ControlChartKind::Individuals {
            "Measurement"
        } else {
            "Moving range"
        }
        .into(),
        reference_lines,
        metadata: PlotMetadata {
            observations: rows.len(),
            displayed: rows.len(),
            sampled: false,
        },
    };
    let summary = ControlChartSummary {
        method: kind,
        observations: values.len(),
        plotted: rows.len(),
        center,
        lower,
        upper,
        within_standard_deviation: finite(p.within_sigma() * p.scale)?,
        mean_moving_range: finite(p.mean_moving_range * p.scale)?,
        outside_count: rows.iter().filter(|r| r.outside).count(),
    };
    Ok(ControlChartResult {
        summary,
        plot,
        rows,
    })
}
