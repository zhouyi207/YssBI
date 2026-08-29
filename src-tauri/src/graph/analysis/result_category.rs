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
    OlsSummary,
    BinarySummary,
    Iv2slsSummary,
    IvLimlSummary,
    PraisSummary,
    VarSummary,
    VarSoc,
    PanelSummary,
    PanelDid,
    DfAdfSummary,
    DfAdfSummaryList,
    VecSummary,
    VecRankSummary,
}

pub(crate) fn result_category_for_output(
    node_type_id: &str,
    port_key: &str,
) -> GraphResultCategory {
    let report = match (node_type_id, port_key) {
        (
            "yssbi.statistics.ols.summary"
            | "yssbi.statistics.gls.summary"
            | "yssbi.statistics.wls.summary",
            "report",
        ) => GraphStatisticalReportKind::OlsSummary,
        ("yssbi.statistics.logit.summary" | "yssbi.statistics.probit.summary", "report") => {
            GraphStatisticalReportKind::BinarySummary
        }
        ("yssbi.statistics.iv.2sls.summary", "report") => GraphStatisticalReportKind::Iv2slsSummary,
        ("yssbi.statistics.iv.liml.summary", "report") => GraphStatisticalReportKind::IvLimlSummary,
        ("yssbi.statistics.prais.summary", "report") => GraphStatisticalReportKind::PraisSummary,
        ("yssbi.statistics.var.summary", "report") => GraphStatisticalReportKind::VarSummary,
        ("yssbi.statistics.var.lag_order", "result") => GraphStatisticalReportKind::VarSoc,
        ("yssbi.statistics.panel.summary", "report") => GraphStatisticalReportKind::PanelSummary,
        ("yssbi.statistics.panel.did.twfe", "report") => GraphStatisticalReportKind::PanelDid,
        ("yssbi.statistics.adf.summary", "report") => GraphStatisticalReportKind::DfAdfSummary,
        ("yssbi.statistics.vec.fit", "model") => GraphStatisticalReportKind::VecSummary,
        ("yssbi.statistics.vec.rank_test", "result") => GraphStatisticalReportKind::VecRankSummary,
        _ => return plot_category_for_output(node_type_id, port_key),
    };
    GraphResultCategory::StatisticalReport(report)
}

pub(crate) fn result_category_for_node(node_type_id: &str) -> GraphResultCategory {
    let output_key = match node_type_id {
        "yssbi.statistics.ols.summary"
        | "yssbi.statistics.gls.summary"
        | "yssbi.statistics.wls.summary"
        | "yssbi.statistics.logit.summary"
        | "yssbi.statistics.probit.summary"
        | "yssbi.statistics.iv.2sls.summary"
        | "yssbi.statistics.iv.liml.summary"
        | "yssbi.statistics.prais.summary"
        | "yssbi.statistics.var.summary"
        | "yssbi.statistics.panel.summary"
        | "yssbi.statistics.panel.did.twfe"
        | "yssbi.statistics.adf.summary" => "report",
        "yssbi.statistics.var.lag_order"
        | "yssbi.plot.scatter.view"
        | "yssbi.plot.line.view"
        | "yssbi.plot.ecdf.view"
        | "yssbi.plot.kde.view"
        | "yssbi.plot.histogram.view"
        | "yssbi.plot.correlation.view"
        | "yssbi.plot.correlogram.view" => "result",
        "yssbi.statistics.vec.fit" => "model",
        "yssbi.statistics.vec.rank_test" => "result",
        _ => "result",
    };
    result_category_for_output(node_type_id, output_key)
}

fn plot_category_for_output(node_type_id: &str, port_key: &str) -> GraphResultCategory {
    if port_key != "result" {
        return GraphResultCategory::Value;
    }
    let plot = match node_type_id {
        "yssbi.plot.scatter.view" => GraphPlotDataKind::Scatter,
        "yssbi.plot.line.view" => GraphPlotDataKind::Line,
        "yssbi.plot.ecdf.view" => GraphPlotDataKind::Ecdf,
        "yssbi.plot.kde.view" => GraphPlotDataKind::Kde,
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
        let report_cases = [
            (
                "yssbi.statistics.ols.summary",
                "report",
                GraphStatisticalReportKind::OlsSummary,
            ),
            (
                "yssbi.statistics.gls.summary",
                "report",
                GraphStatisticalReportKind::OlsSummary,
            ),
            (
                "yssbi.statistics.wls.summary",
                "report",
                GraphStatisticalReportKind::OlsSummary,
            ),
            (
                "yssbi.statistics.logit.summary",
                "report",
                GraphStatisticalReportKind::BinarySummary,
            ),
            (
                "yssbi.statistics.probit.summary",
                "report",
                GraphStatisticalReportKind::BinarySummary,
            ),
            (
                "yssbi.statistics.iv.2sls.summary",
                "report",
                GraphStatisticalReportKind::Iv2slsSummary,
            ),
            (
                "yssbi.statistics.iv.liml.summary",
                "report",
                GraphStatisticalReportKind::IvLimlSummary,
            ),
            (
                "yssbi.statistics.prais.summary",
                "report",
                GraphStatisticalReportKind::PraisSummary,
            ),
            (
                "yssbi.statistics.var.summary",
                "report",
                GraphStatisticalReportKind::VarSummary,
            ),
            (
                "yssbi.statistics.var.lag_order",
                "result",
                GraphStatisticalReportKind::VarSoc,
            ),
            (
                "yssbi.statistics.panel.summary",
                "report",
                GraphStatisticalReportKind::PanelSummary,
            ),
            (
                "yssbi.statistics.panel.did.twfe",
                "report",
                GraphStatisticalReportKind::PanelDid,
            ),
            (
                "yssbi.statistics.adf.summary",
                "report",
                GraphStatisticalReportKind::DfAdfSummary,
            ),
            (
                "yssbi.statistics.vec.fit",
                "model",
                GraphStatisticalReportKind::VecSummary,
            ),
            (
                "yssbi.statistics.vec.rank_test",
                "result",
                GraphStatisticalReportKind::VecRankSummary,
            ),
        ];
        for (node_type, output_key, report) in report_cases {
            assert_eq!(
                result_category_for_output(node_type, output_key),
                GraphResultCategory::StatisticalReport(report),
                "{node_type}:{output_key}",
            );
        }

        let plot_cases = [
            ("yssbi.plot.scatter.view", GraphPlotDataKind::Scatter),
            ("yssbi.plot.line.view", GraphPlotDataKind::Line),
            ("yssbi.plot.ecdf.view", GraphPlotDataKind::Ecdf),
            ("yssbi.plot.kde.view", GraphPlotDataKind::Kde),
            ("yssbi.plot.histogram.view", GraphPlotDataKind::Histogram),
            (
                "yssbi.plot.correlation.view",
                GraphPlotDataKind::Correlation,
            ),
            (
                "yssbi.plot.correlogram.view",
                GraphPlotDataKind::Correlogram,
            ),
        ];
        for (node_type, plot) in plot_cases {
            assert_eq!(
                result_category_for_output(node_type, "result"),
                GraphResultCategory::PlotData(plot),
                "{node_type}:result",
            );
        }

        for output_key in ["fitted", "residuals", "report"] {
            assert_eq!(
                result_category_for_output("yssbi.statistics.vec.fit", output_key),
                GraphResultCategory::Value,
                "vec.fit:{output_key}",
            );
        }
        assert_eq!(
            result_category_for_output("yssbi.plot.scatter.view", "other"),
            GraphResultCategory::Value,
        );
    }
}
