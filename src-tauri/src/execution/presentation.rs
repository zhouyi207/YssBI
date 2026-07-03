use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlotChart {
    Scatter,
    Line,
    Plot,
    Ecdf,
    Kde,
    Histogram,
    Correlation,
    Correlogram,
}

impl PlotChart {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scatter => "scatter",
            Self::Line => "line",
            Self::Plot => "plot",
            Self::Ecdf => "ecdf",
            Self::Kde => "kde",
            Self::Histogram => "histogram",
            Self::Correlation => "correlation",
            Self::Correlogram => "correlogram",
        }
    }

    pub fn default_title(self) -> &'static str {
        match self {
            Self::Scatter => "Scatter Plot",
            Self::Line => "Line Plot",
            Self::Plot => "Plot",
            Self::Ecdf => "ECDF Plot",
            Self::Kde => "KDE Plot",
            Self::Histogram => "Histogram",
            Self::Correlation => "Correlation Plot",
            Self::Correlogram => "Correlogram",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReportKind {
    OlsSummary,
    VarSummary,
    VarSoc,
    PanelSummary,
    PanelDid,
    DfAdfSummary,
    DfAdfSummaryList,
    VecSummary,
    VecRankSummary,
}

impl ReportKind {
    pub fn default_title(self) -> &'static str {
        match self {
            Self::OlsSummary => "Results",
            Self::VarSummary => "VAR Summary",
            Self::VarSoc => "VAR SOC",
            Self::PanelSummary => "Panel Summary",
            Self::PanelDid => "Panel DID",
            Self::DfAdfSummary => "ADF Summary",
            Self::DfAdfSummaryList => "ADF Summary List",
            Self::VecSummary => "VEC Summary",
            Self::VecRankSummary => "VECRANK Summary",
        }
    }

    pub fn from_legacy_key(key: &str) -> Self {
        match key {
            "var_summary" => Self::VarSummary,
            "var_soc" => Self::VarSoc,
            "panel_summary" => Self::PanelSummary,
            "panel_did" => Self::PanelDid,
            "df_adf_summary" => Self::DfAdfSummary,
            "df_adf_summary_list" => Self::DfAdfSummaryList,
            "vec_summary" => Self::VecSummary,
            "vec_rank_summary" => Self::VecRankSummary,
            _ => Self::OlsSummary,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Presentation {
    Inspector,
    Plot { chart: PlotChart },
    Report { report: ReportKind },
}

impl Presentation {
    pub fn route(self) -> &'static str {
        match self {
            Self::Inspector => "/inspect",
            Self::Plot { .. } => "/plot",
            Self::Report { .. } => "/info",
        }
    }

    pub fn plot_chart(self) -> Option<PlotChart> {
        match self {
            Self::Plot { chart } => Some(chart),
            _ => None,
        }
    }

    pub fn default_title(self) -> &'static str {
        match self {
            Self::Inspector => "Source Inspector",
            Self::Plot { chart } => chart.default_title(),
            Self::Report { report } => report.default_title(),
        }
    }
}
