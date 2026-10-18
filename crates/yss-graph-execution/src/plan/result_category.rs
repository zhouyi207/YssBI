#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultCategory {
    Value,
    PlotData(PlotDataKind),
    StatisticalReport(StatisticalReportKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlotDataKind {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatisticalReportKind {
    Structured,
    LinearRegressionSummary,
}

impl PlotDataKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Scatter => "scatter",
            Self::Line => "line",
            Self::Ecdf => "ecdf",
            Self::Kde => "kde",
            Self::Histogram => "histogram",
            Self::Correlation => "correlation",
            Self::Correlogram => "correlogram",
            Self::Boxplot => "boxplot",
            Self::Wordcloud => "wordcloud",
            Self::Errorbar => "errorbar",
            Self::PpQq => "pp_qq",
            Self::Roc => "roc",
            Self::Quadrant => "quadrant",
            Self::Pareto => "pareto",
            Self::Combination => "combination",
            Self::Bubble => "bubble",
            Self::Violin => "violin",
            Self::Heatmap => "heatmap",
            Self::Coefficient => "coefficient",
            Self::Nomogram => "nomogram",
        }
    }
}

impl StatisticalReportKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Structured => "structured",
            Self::LinearRegressionSummary => "linear_regression_summary",
        }
    }
}
