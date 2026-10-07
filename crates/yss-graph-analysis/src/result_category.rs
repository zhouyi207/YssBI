use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphResultCategory {
    Value,
    PlotData(GraphPlotDataKind),
    StatisticalReport(GraphStatisticalReportKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphPlotDataKind {
    Scatter,
    Line,
    Ecdf,
    Kde,
    Histogram,
    Correlation,
    Correlogram,
    Boxplot,
    Wordcloud,
    Errorbar,
    PpQq,
    Roc,
    Quadrant,
    Pareto,
    Combination,
    Bubble,
    Violin,
    Heatmap,
    Coefficient,
    Nomogram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphStatisticalReportKind {
    Structured,
    LinearRegressionSummary,
}

pub(crate) fn result_category_for_output(
    node_type_id: &str,
    port_key: &str,
) -> GraphResultCategory {
    if port_key == "result" && node_type_id == "yssbi.statistics.linear.summary" {
        GraphResultCategory::StatisticalReport(GraphStatisticalReportKind::LinearRegressionSummary)
    } else if node_type_id == "yssbi.statistics.plot.control_chart" && port_key == "summary" {
        GraphResultCategory::StatisticalReport(GraphStatisticalReportKind::Structured)
    } else if matches!(
        node_type_id,
        "yssbi.statistics.plot.time_series"
            | "yssbi.statistics.plot.correlogram"
            | "yssbi.statistics.plot.nomogram"
            | "yssbi.statistics.plot.calibration"
            | "yssbi.statistics.plot.decision_curve"
            | "yssbi.statistics.plot.forest"
            | "yssbi.statistics.plot.funnel"
            | "yssbi.statistics.plot.control_chart"
    ) {
        plot_category_for_output(node_type_id, port_key)
    } else if port_key == "result" && node_type_id.starts_with("yssbi.statistics.") {
        GraphResultCategory::StatisticalReport(GraphStatisticalReportKind::Structured)
    } else {
        plot_category_for_output(node_type_id, port_key)
    }
}

fn plot_category_for_output(node_type_id: &str, port_key: &str) -> GraphResultCategory {
    if port_key != "result" {
        return GraphResultCategory::Value;
    }
    let plot = match node_type_id {
        "yssbi.plot.scatter.view" | "yssbi.statistics.plot.funnel" => GraphPlotDataKind::Scatter,
        "yssbi.plot.line.view"
        | "yssbi.statistics.plot.time_series"
        | "yssbi.statistics.plot.control_chart" => GraphPlotDataKind::Line,
        "yssbi.plot.ecdf.view" => GraphPlotDataKind::Ecdf,
        "yssbi.plot.kde.view" => GraphPlotDataKind::Kde,
        "yssbi.plot.boxplot.view" => GraphPlotDataKind::Boxplot,
        "yssbi.plot.wordcloud.view" => GraphPlotDataKind::Wordcloud,
        "yssbi.plot.errorbar.view" => GraphPlotDataKind::Errorbar,
        "yssbi.plot.pp_qq.view" => GraphPlotDataKind::PpQq,
        "yssbi.plot.roc.view" => GraphPlotDataKind::Roc,
        "yssbi.plot.quadrant.view" => GraphPlotDataKind::Quadrant,
        "yssbi.plot.pareto.view" => GraphPlotDataKind::Pareto,
        "yssbi.plot.combination.view" => GraphPlotDataKind::Combination,
        "yssbi.plot.bubble.view" => GraphPlotDataKind::Bubble,
        "yssbi.plot.violin.view" => GraphPlotDataKind::Violin,
        "yssbi.plot.heatmap.view" => GraphPlotDataKind::Heatmap,
        "yssbi.plot.coefficient.view" | "yssbi.statistics.plot.forest" => {
            GraphPlotDataKind::Coefficient
        }
        "yssbi.statistics.plot.nomogram" => GraphPlotDataKind::Nomogram,
        "yssbi.statistics.plot.calibration" | "yssbi.statistics.plot.decision_curve" => {
            GraphPlotDataKind::Line
        }
        "yssbi.plot.histogram.view" => GraphPlotDataKind::Histogram,
        "yssbi.plot.correlation.view" => GraphPlotDataKind::Correlation,
        "yssbi.plot.correlogram.view" | "yssbi.statistics.plot.correlogram" => {
            GraphPlotDataKind::Correlogram
        }
        _ => return GraphResultCategory::Value,
    };
    GraphResultCategory::PlotData(plot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_output_categories_are_output_specific() {
        assert_eq!(
            result_category_for_output("yssbi.statistics.linear.summary", "result"),
            GraphResultCategory::StatisticalReport(
                GraphStatisticalReportKind::LinearRegressionSummary
            ),
        );
        for node in [
            "yssbi.statistics.describe",
            "yssbi.statistics.logit.summary",
            "yssbi.statistics.probit.summary",
            "yssbi.statistics.prais.summary",
            "yssbi.statistics.iv.2sls.summary",
            "yssbi.statistics.iv.liml.summary",
            "yssbi.statistics.panel.summary",
            "yssbi.statistics.panel.did.twfe",
            "yssbi.statistics.panel.did.randomization",
            "yssbi.statistics.var.summary",
            "yssbi.statistics.var.lag_order",
            "yssbi.statistics.vec.summary",
            "yssbi.statistics.vec.rank_test",
            "yssbi.statistics.adf.test",
            "yssbi.statistics.diagnostic.reset",
            "yssbi.statistics.inequality.theil",
        ] {
            assert_eq!(
                result_category_for_output(node, "result"),
                GraphResultCategory::StatisticalReport(GraphStatisticalReportKind::Structured),
                "{node}"
            );
        }
        for (node, kind) in [
            ("yssbi.plot.scatter.view", GraphPlotDataKind::Scatter),
            ("yssbi.statistics.plot.funnel", GraphPlotDataKind::Scatter),
            (
                "yssbi.statistics.plot.forest",
                GraphPlotDataKind::Coefficient,
            ),
            (
                "yssbi.statistics.plot.nomogram",
                GraphPlotDataKind::Nomogram,
            ),
            ("yssbi.statistics.plot.calibration", GraphPlotDataKind::Line),
            (
                "yssbi.statistics.plot.decision_curve",
                GraphPlotDataKind::Line,
            ),
            ("yssbi.plot.line.view", GraphPlotDataKind::Line),
            (
                "yssbi.statistics.plot.control_chart",
                GraphPlotDataKind::Line,
            ),
            ("yssbi.plot.ecdf.view", GraphPlotDataKind::Ecdf),
            ("yssbi.plot.kde.view", GraphPlotDataKind::Kde),
            ("yssbi.plot.boxplot.view", GraphPlotDataKind::Boxplot),
            ("yssbi.plot.wordcloud.view", GraphPlotDataKind::Wordcloud),
            ("yssbi.plot.errorbar.view", GraphPlotDataKind::Errorbar),
            ("yssbi.plot.pp_qq.view", GraphPlotDataKind::PpQq),
            ("yssbi.plot.roc.view", GraphPlotDataKind::Roc),
            ("yssbi.plot.quadrant.view", GraphPlotDataKind::Quadrant),
            ("yssbi.plot.pareto.view", GraphPlotDataKind::Pareto),
            (
                "yssbi.plot.combination.view",
                GraphPlotDataKind::Combination,
            ),
            ("yssbi.plot.bubble.view", GraphPlotDataKind::Bubble),
            ("yssbi.plot.violin.view", GraphPlotDataKind::Violin),
            ("yssbi.plot.heatmap.view", GraphPlotDataKind::Heatmap),
            (
                "yssbi.plot.coefficient.view",
                GraphPlotDataKind::Coefficient,
            ),
            ("yssbi.plot.histogram.view", GraphPlotDataKind::Histogram),
            (
                "yssbi.plot.correlation.view",
                GraphPlotDataKind::Correlation,
            ),
            (
                "yssbi.plot.correlogram.view",
                GraphPlotDataKind::Correlogram,
            ),
        ] {
            assert_eq!(
                result_category_for_output(node, "result"),
                GraphResultCategory::PlotData(kind)
            );
        }
        for output in ["model", "fitted", "residuals"] {
            assert_eq!(
                result_category_for_output("yssbi.statistics.linear.fit", output),
                GraphResultCategory::Value
            );
        }
        assert_eq!(
            result_category_for_output("yssbi.statistics.plot.control_chart", "summary"),
            GraphResultCategory::StatisticalReport(GraphStatisticalReportKind::Structured)
        );
        assert_eq!(
            result_category_for_output("yssbi.statistics.plot.control_chart", "observations"),
            GraphResultCategory::Value
        );
        assert_eq!(
            result_category_for_output("yssbi.plot.scatter.view", "other"),
            GraphResultCategory::Value
        );
    }
}
