//! Stable inventory for the DataFrame catalog.

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum InterfaceKind {
    DataframeSource,
    Limit,
    Rename,
    Project,
    FilterRows,
    DropColumns,
    DropRows,
    DropNaRows,
    DropNaColumns,
    Decompose,
    Combine,
    ConcatRows,
    ConcatColumns,
    Join,
    SeriesSelect,
    IntRange,
    SeriesLength,
    SeriesCount,
    SeriesSum,
    SeriesMean,
    Frequency,
    GroupBy,
    Standardize,
    InverseStandardize,
    DummyInfo,
    TimeAlign,
    TimeUnary,
    TimeWindow,
    TimeLag,
    PanelAlign,
    PanelDifference,
}

#[derive(Clone, Copy)]
pub(super) struct NodeSpec {
    pub id: &'static str,
    pub interface: InterfaceKind,
}

pub(super) const NODES: &[NodeSpec] = &[
    spec("yssbi.dataframe.series.frequency", InterfaceKind::Frequency),
    spec("yssbi.dataframe.groupby", InterfaceKind::GroupBy),
    spec("yssbi.dataframe.concat.rows", InterfaceKind::ConcatRows),
    spec(
        "yssbi.dataframe.concat.columns",
        InterfaceKind::ConcatColumns,
    ),
    spec("yssbi.dataframe.join", InterfaceKind::Join),
    spec(
        crate::DATAFRAME_SOURCE_NODE_TYPE,
        InterfaceKind::DataframeSource,
    ),
    spec("yssbi.dataframe.limit", InterfaceKind::Limit),
    spec("yssbi.dataframe.rename", InterfaceKind::Rename),
    spec("yssbi.dataframe.project", InterfaceKind::Project),
    spec("yssbi.dataframe.filter.rows", InterfaceKind::FilterRows),
    spec("yssbi.dataframe.drop.columns", InterfaceKind::DropColumns),
    spec("yssbi.dataframe.drop.rows", InterfaceKind::DropRows),
    spec("yssbi.dataframe.dropna.rows", InterfaceKind::DropNaRows),
    spec(
        "yssbi.dataframe.dropna.columns",
        InterfaceKind::DropNaColumns,
    ),
    spec("yssbi.dataframe.decompose", InterfaceKind::Decompose),
    spec("yssbi.dataframe.combine", InterfaceKind::Combine),
    spec("yssbi.dataframe.series.select", InterfaceKind::SeriesSelect),
    spec("yssbi.dataframe.series.int_range", InterfaceKind::IntRange),
    spec("yssbi.dataframe.series.length", InterfaceKind::SeriesLength),
    spec("yssbi.dataframe.series.count", InterfaceKind::SeriesCount),
    spec("yssbi.dataframe.series.sum", InterfaceKind::SeriesSum),
    spec("yssbi.dataframe.series.mean", InterfaceKind::SeriesMean),
    spec(
        "yssbi.dataframe.series.standardize",
        InterfaceKind::Standardize,
    ),
    spec(
        "yssbi.dataframe.series.inverse_standardize",
        InterfaceKind::InverseStandardize,
    ),
    spec(
        "yssbi.dataframe.series.annotate_dummy",
        InterfaceKind::DummyInfo,
    ),
    spec("yssbi.dataframe.timeseries.align", InterfaceKind::TimeAlign),
    spec(
        "yssbi.dataframe.timeseries.difference",
        InterfaceKind::TimeUnary,
    ),
    spec(
        "yssbi.dataframe.timeseries.percent_change",
        InterfaceKind::TimeUnary,
    ),
    spec(
        "yssbi.dataframe.timeseries.rolling_mean",
        InterfaceKind::TimeWindow,
    ),
    spec("yssbi.dataframe.timeseries.lag", InterfaceKind::TimeLag),
    spec("yssbi.dataframe.panel.align", InterfaceKind::PanelAlign),
    spec(
        "yssbi.dataframe.panel.difference",
        InterfaceKind::PanelDifference,
    ),
];

const fn spec(id: &'static str, interface: InterfaceKind) -> NodeSpec {
    NodeSpec { id, interface }
}
