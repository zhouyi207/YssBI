use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultCategory {
    Value,
    PlotData(PlotDataKind),
    StatisticalReport(StatisticalReportKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlotDataKind {
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
pub enum StatisticalReportKind {
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
