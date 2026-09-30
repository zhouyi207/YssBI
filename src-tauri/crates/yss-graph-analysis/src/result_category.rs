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
    Plot,
    Ecdf,
    Kde,
    Histogram,
    Correlation,
    Correlogram,
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
    } else if port_key == "result"
        && (node_type_id.starts_with("yssbi.statistics.") || node_type_id == "yssbi.plot.kde.view")
    {
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
        "yssbi.plot.scatter.view" => GraphPlotDataKind::Scatter,
        "yssbi.plot.line.view" => GraphPlotDataKind::Line,
        "yssbi.plot.ecdf.view" => GraphPlotDataKind::Ecdf,
        "yssbi.plot.histogram.view" => GraphPlotDataKind::Histogram,
        "yssbi.plot.correlation.view" => GraphPlotDataKind::Correlation,
        "yssbi.plot.correlogram.view" => GraphPlotDataKind::Correlogram,
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
            "yssbi.plot.kde.view",
        ] {
            assert_eq!(
                result_category_for_output(node, "result"),
                GraphResultCategory::StatisticalReport(GraphStatisticalReportKind::Structured),
                "{node}"
            );
        }
        for (node, kind) in [
            ("yssbi.plot.scatter.view", GraphPlotDataKind::Scatter),
            ("yssbi.plot.line.view", GraphPlotDataKind::Line),
            ("yssbi.plot.ecdf.view", GraphPlotDataKind::Ecdf),
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
            result_category_for_output("yssbi.plot.scatter.view", "other"),
            GraphResultCategory::Value
        );
    }
}
