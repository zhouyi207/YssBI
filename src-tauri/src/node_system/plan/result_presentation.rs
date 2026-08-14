use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResultPlotKind {
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
#[serde(rename_all = "camelCase")]
pub enum ResultReportKind {
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

impl ResultReportKind {
    pub const fn canonical_title(self) -> &'static str {
        match self {
            Self::OlsSummary => "OLS Summary",
            Self::BinarySummary => "Binary Model Summary",
            Self::Iv2slsSummary => "IV 2SLS Summary",
            Self::IvLimlSummary => "IV LIML Summary",
            Self::PraisSummary => "Prais-Winsten Summary",
            Self::VarSummary => "VAR Summary",
            Self::VarSoc => "VAR Lag Order",
            Self::PanelSummary => "Panel Model Summary",
            Self::PanelDid => "Panel Difference-in-Differences",
            Self::DfAdfSummary => "ADF Summary",
            Self::DfAdfSummaryList => "ADF Summaries",
            Self::VecSummary => "VEC Summary",
            Self::VecRankSummary => "VEC Rank Summary",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResultPresentation {
    #[default]
    Inspector,
    Plot {
        chart: ResultPlotKind,
    },
    Report {
        report: ResultReportKind,
    },
}

pub(crate) fn presentation_for_output(node_type_id: &str, port_key: &str) -> ResultPresentation {
    let report = match (node_type_id, port_key) {
        (
            "yssbi.statistics.ols.summary"
            | "yssbi.statistics.gls.summary"
            | "yssbi.statistics.wls.summary",
            "report",
        ) => ResultReportKind::OlsSummary,
        ("yssbi.statistics.logit.summary" | "yssbi.statistics.probit.summary", "report") => {
            ResultReportKind::BinarySummary
        }
        ("yssbi.statistics.iv.2sls.summary", "report") => ResultReportKind::Iv2slsSummary,
        ("yssbi.statistics.iv.liml.summary", "report") => ResultReportKind::IvLimlSummary,
        ("yssbi.statistics.prais.summary", "report") => ResultReportKind::PraisSummary,
        ("yssbi.statistics.var.summary", "report") => ResultReportKind::VarSummary,
        ("yssbi.statistics.var.lag_order", "result") => ResultReportKind::VarSoc,
        ("yssbi.statistics.panel.summary", "report") => ResultReportKind::PanelSummary,
        ("yssbi.statistics.panel.did.twfe", "report") => ResultReportKind::PanelDid,
        ("yssbi.statistics.adf.summary", "report") => ResultReportKind::DfAdfSummary,
        ("yssbi.statistics.vec.fit", "model") => ResultReportKind::VecSummary,
        ("yssbi.statistics.vec.rank_test", "result") => ResultReportKind::VecRankSummary,
        _ => return plot_presentation(node_type_id, port_key),
    };
    ResultPresentation::Report { report }
}

fn plot_presentation(node_type_id: &str, port_key: &str) -> ResultPresentation {
    if port_key != "result" {
        return ResultPresentation::Inspector;
    }
    let chart = match node_type_id {
        "yssbi.plot.scatter.view" => ResultPlotKind::Scatter,
        "yssbi.plot.line.view" => ResultPlotKind::Line,
        "yssbi.plot.ecdf.view" => ResultPlotKind::Ecdf,
        "yssbi.plot.kde.view" => ResultPlotKind::Kde,
        "yssbi.plot.histogram.view" => ResultPlotKind::Histogram,
        "yssbi.plot.correlation.view" => ResultPlotKind::Correlation,
        "yssbi.plot.correlogram.view" => ResultPlotKind::Correlogram,
        _ => return ResultPresentation::Inspector,
    };
    ResultPresentation::Plot { chart }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_output_presentations_are_output_specific() {
        let report_cases = [
            (
                "yssbi.statistics.ols.summary",
                "report",
                ResultReportKind::OlsSummary,
            ),
            (
                "yssbi.statistics.gls.summary",
                "report",
                ResultReportKind::OlsSummary,
            ),
            (
                "yssbi.statistics.wls.summary",
                "report",
                ResultReportKind::OlsSummary,
            ),
            (
                "yssbi.statistics.logit.summary",
                "report",
                ResultReportKind::BinarySummary,
            ),
            (
                "yssbi.statistics.probit.summary",
                "report",
                ResultReportKind::BinarySummary,
            ),
            (
                "yssbi.statistics.iv.2sls.summary",
                "report",
                ResultReportKind::Iv2slsSummary,
            ),
            (
                "yssbi.statistics.iv.liml.summary",
                "report",
                ResultReportKind::IvLimlSummary,
            ),
            (
                "yssbi.statistics.prais.summary",
                "report",
                ResultReportKind::PraisSummary,
            ),
            (
                "yssbi.statistics.var.summary",
                "report",
                ResultReportKind::VarSummary,
            ),
            (
                "yssbi.statistics.var.lag_order",
                "result",
                ResultReportKind::VarSoc,
            ),
            (
                "yssbi.statistics.panel.summary",
                "report",
                ResultReportKind::PanelSummary,
            ),
            (
                "yssbi.statistics.panel.did.twfe",
                "report",
                ResultReportKind::PanelDid,
            ),
            (
                "yssbi.statistics.adf.summary",
                "report",
                ResultReportKind::DfAdfSummary,
            ),
            (
                "yssbi.statistics.vec.fit",
                "model",
                ResultReportKind::VecSummary,
            ),
            (
                "yssbi.statistics.vec.rank_test",
                "result",
                ResultReportKind::VecRankSummary,
            ),
        ];
        for (node_type, output_key, report) in report_cases {
            assert_eq!(
                presentation_for_output(node_type, output_key),
                ResultPresentation::Report { report },
                "{node_type}:{output_key}",
            );
        }

        let plot_cases = [
            ("yssbi.plot.scatter.view", ResultPlotKind::Scatter),
            ("yssbi.plot.line.view", ResultPlotKind::Line),
            ("yssbi.plot.ecdf.view", ResultPlotKind::Ecdf),
            ("yssbi.plot.kde.view", ResultPlotKind::Kde),
            ("yssbi.plot.histogram.view", ResultPlotKind::Histogram),
            ("yssbi.plot.correlation.view", ResultPlotKind::Correlation),
            ("yssbi.plot.correlogram.view", ResultPlotKind::Correlogram),
        ];
        for (node_type, chart) in plot_cases {
            assert_eq!(
                presentation_for_output(node_type, "result"),
                ResultPresentation::Plot { chart },
                "{node_type}:result",
            );
        }

        for output_key in ["fitted", "residuals", "report"] {
            assert_eq!(
                presentation_for_output("yssbi.statistics.vec.fit", output_key),
                ResultPresentation::Inspector,
                "vec.fit:{output_key}",
            );
        }
        assert_eq!(
            presentation_for_output("yssbi.plot.scatter.view", "other"),
            ResultPresentation::Inspector,
        );
    }
}
