//! Generate real numerical plot payloads for contract checks and browser previews.
use serde::Serialize;
use std::time::{Duration, Instant};
use yss_sci::visualization as plot;
use yss_sci_contract::execution::{ScientificCancellationToken, ScientificExecutionControl};
use yss_sci_contract::visualization::ProbabilityPlotMode;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let control = ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    };
    let x = vec![1., 2., 3., 4., 5., 6., 7., 8.];
    let y = vec![2., 2.9, 4.2, 3.8, 5.1, 6.2, 5.9, 7.3];
    let groups = vec![
        vec![1., 2., 2., 3., 4., 4., 5., 12.],
        vec![3., 3.5, 4., 5., 5.5, 6., 7., 8.],
    ];
    let names = vec!["A".to_string(), "B".to_string()];
    let categories = [
        "Alpha", "Beta", "Alpha", "Gamma", "Alpha", "Beta", "Delta", "Beta",
    ]
    .map(String::from);
    let words = [
        "Data", "Science", "Rust", "Science", "D3", "Data", "Data", "Graph", "数据", "数据",
        "统计", "模型", "模型",
    ]
    .map(String::from);
    let mut payloads = Vec::new();
    fn add<T: Serialize>(
        output: &mut Vec<serde_json::Value>,
        kind: &str,
        data: T,
    ) -> Result<(), serde_json::Error> {
        output.push(serde_json::json!({ "chart": kind, "data": data }));
        Ok(())
    }
    add(&mut payloads, "scatter", plot::xy(&x, &y, false, &control)?)?;
    add(&mut payloads, "line", plot::xy(&x, &y, true, &control)?)?;
    add(&mut payloads, "ecdf", plot::ecdf(&groups[0], &control)?)?;
    add(&mut payloads, "kde", plot::kde(&groups[0], 256, &control)?)?;
    add(
        &mut payloads,
        "histogram",
        plot::histogram(&groups[0], 5, &control)?,
    )?;
    add(
        &mut payloads,
        "correlation",
        plot::correlation(&names, &[x.clone(), y.clone()], &control)?,
    )?;
    add(
        &mut payloads,
        "correlogram",
        plot::correlogram(&y, 3, &control)?,
    )?;
    add(
        &mut payloads,
        "boxplot",
        plot::box_violin(&names, &groups, false, &control)?,
    )?;
    add(
        &mut payloads,
        "violin",
        plot::box_violin(&names, &groups, true, &control)?,
    )?;
    add(
        &mut payloads,
        "wordcloud",
        plot::word_cloud(&words, 100, &control)?,
    )?;
    add(
        &mut payloads,
        "errorbar",
        plot::error_bars(
            &x,
            &y,
            &y.iter().map(|v| v - 0.7).collect::<Vec<_>>(),
            &y.iter().map(|v| v + 0.7).collect::<Vec<_>>(),
            &control,
        )?,
    )?;
    add(
        &mut payloads,
        "ppQq",
        plot::probability(&groups[1], ProbabilityPlotMode::Qq, true, 0., 1., &control)?,
    )?;
    add(
        &mut payloads,
        "roc",
        plot::roc(
            &[false, false, true, false, true, true, false, true],
            &[0.05, 0.2, 0.3, 0.3, 0.5, 0.8, 0.6, 0.9],
            &control,
        )?,
    )?;
    add(
        &mut payloads,
        "quadrant",
        plot::quadrant(&x, &y, 4., 4., &control)?,
    )?;
    add(
        &mut payloads,
        "pareto",
        plot::pareto(&categories, &control)?,
    )?;
    add(
        &mut payloads,
        "combination",
        plot::combination(&categories, &x, &y, true, &control)?,
    )?;
    add(
        &mut payloads,
        "bubble",
        plot::bubble(&x, &y, &[1., 2., 4., 8., 3., 2., 5., 7.], &control)?,
    )?;
    add(
        &mut payloads,
        "heatmap",
        plot::heatmap(&names, &groups, &control)?,
    )?;
    add(
        &mut payloads,
        "coefficient",
        plot::coefficients(
            &["X1".into(), "X2".into(), "X3".into()],
            &[0.8, -0.4, 0.1],
            &[0.1, 0.15, 0.1],
            30.,
            0.95,
            &control,
        )?,
    )?;
    let document = serde_json::json!({ "payloads": payloads });
    if let Some(path) = std::env::args().nth(1) {
        serde_json::to_writer_pretty(std::fs::File::create(path)?, &document)?;
    } else {
        serde_json::to_writer_pretty(std::io::stdout(), &document)?;
    }
    Ok(())
}
