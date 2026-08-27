use crate::execution::plan::result_category::{
    PlotDataKind, ResultCategory, StatisticalReportKind,
};

pub(crate) fn result_category_for_output(node_type_id: &str, port_key: &str) -> ResultCategory {
    let report = match (node_type_id, port_key) {
        (
            "yssbi.statistics.ols.summary"
            | "yssbi.statistics.gls.summary"
            | "yssbi.statistics.wls.summary",
            "report",
        ) => StatisticalReportKind::OlsSummary,
        ("yssbi.statistics.logit.summary" | "yssbi.statistics.probit.summary", "report") => {
            StatisticalReportKind::BinarySummary
        }
        ("yssbi.statistics.iv.2sls.summary", "report") => StatisticalReportKind::Iv2slsSummary,
        ("yssbi.statistics.iv.liml.summary", "report") => StatisticalReportKind::IvLimlSummary,
        ("yssbi.statistics.prais.summary", "report") => StatisticalReportKind::PraisSummary,
        ("yssbi.statistics.var.summary", "report") => StatisticalReportKind::VarSummary,
        ("yssbi.statistics.var.lag_order", "result") => StatisticalReportKind::VarSoc,
        ("yssbi.statistics.panel.summary", "report") => StatisticalReportKind::PanelSummary,
        ("yssbi.statistics.panel.did.twfe", "report") => StatisticalReportKind::PanelDid,
        ("yssbi.statistics.adf.summary", "report") => StatisticalReportKind::DfAdfSummary,
        ("yssbi.statistics.vec.fit", "model") => StatisticalReportKind::VecSummary,
        ("yssbi.statistics.vec.rank_test", "result") => StatisticalReportKind::VecRankSummary,
        _ => return plot_category_for_output(node_type_id, port_key),
    };
    ResultCategory::StatisticalReport(report)
}

fn plot_category_for_output(node_type_id: &str, port_key: &str) -> ResultCategory {
    if port_key != "result" {
        return ResultCategory::Value;
    }
    let plot = match node_type_id {
        "yssbi.plot.scatter.view" => PlotDataKind::Scatter,
        "yssbi.plot.line.view" => PlotDataKind::Line,
        "yssbi.plot.ecdf.view" => PlotDataKind::Ecdf,
        "yssbi.plot.kde.view" => PlotDataKind::Kde,
        "yssbi.plot.histogram.view" => PlotDataKind::Histogram,
        "yssbi.plot.correlation.view" => PlotDataKind::Correlation,
        "yssbi.plot.correlogram.view" => PlotDataKind::Correlogram,
        _ => return ResultCategory::Value,
    };
    ResultCategory::PlotData(plot)
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
                StatisticalReportKind::OlsSummary,
            ),
            (
                "yssbi.statistics.gls.summary",
                "report",
                StatisticalReportKind::OlsSummary,
            ),
            (
                "yssbi.statistics.wls.summary",
                "report",
                StatisticalReportKind::OlsSummary,
            ),
            (
                "yssbi.statistics.logit.summary",
                "report",
                StatisticalReportKind::BinarySummary,
            ),
            (
                "yssbi.statistics.probit.summary",
                "report",
                StatisticalReportKind::BinarySummary,
            ),
            (
                "yssbi.statistics.iv.2sls.summary",
                "report",
                StatisticalReportKind::Iv2slsSummary,
            ),
            (
                "yssbi.statistics.iv.liml.summary",
                "report",
                StatisticalReportKind::IvLimlSummary,
            ),
            (
                "yssbi.statistics.prais.summary",
                "report",
                StatisticalReportKind::PraisSummary,
            ),
            (
                "yssbi.statistics.var.summary",
                "report",
                StatisticalReportKind::VarSummary,
            ),
            (
                "yssbi.statistics.var.lag_order",
                "result",
                StatisticalReportKind::VarSoc,
            ),
            (
                "yssbi.statistics.panel.summary",
                "report",
                StatisticalReportKind::PanelSummary,
            ),
            (
                "yssbi.statistics.panel.did.twfe",
                "report",
                StatisticalReportKind::PanelDid,
            ),
            (
                "yssbi.statistics.adf.summary",
                "report",
                StatisticalReportKind::DfAdfSummary,
            ),
            (
                "yssbi.statistics.vec.fit",
                "model",
                StatisticalReportKind::VecSummary,
            ),
            (
                "yssbi.statistics.vec.rank_test",
                "result",
                StatisticalReportKind::VecRankSummary,
            ),
        ];
        for (node_type, output_key, report) in report_cases {
            assert_eq!(
                result_category_for_output(node_type, output_key),
                ResultCategory::StatisticalReport(report),
                "{node_type}:{output_key}",
            );
        }

        let plot_cases = [
            ("yssbi.plot.scatter.view", PlotDataKind::Scatter),
            ("yssbi.plot.line.view", PlotDataKind::Line),
            ("yssbi.plot.ecdf.view", PlotDataKind::Ecdf),
            ("yssbi.plot.kde.view", PlotDataKind::Kde),
            ("yssbi.plot.histogram.view", PlotDataKind::Histogram),
            ("yssbi.plot.correlation.view", PlotDataKind::Correlation),
            ("yssbi.plot.correlogram.view", PlotDataKind::Correlogram),
        ];
        for (node_type, plot) in plot_cases {
            assert_eq!(
                result_category_for_output(node_type, "result"),
                ResultCategory::PlotData(plot),
                "{node_type}:result",
            );
        }

        for output_key in ["fitted", "residuals", "report"] {
            assert_eq!(
                result_category_for_output("yssbi.statistics.vec.fit", output_key),
                ResultCategory::Value,
                "vec.fit:{output_key}",
            );
        }
        assert_eq!(
            result_category_for_output("yssbi.plot.scatter.view", "other"),
            ResultCategory::Value,
        );
    }
}
