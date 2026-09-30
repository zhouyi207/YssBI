//! Operation requests for native relational transformations, not a second expression IR.
use crate::{RelationHandle, SeriesOperand};
use yss_data_contract::TabularScalar;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortColumn {
    pub column: Box<str>,
    pub ascending: bool,
    pub nulls_first: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DuplicateKeep {
    First,
    Last,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowOperation {
    Sum,
    Mean,
    Min,
    Max,
    StandardDeviation,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeriesWindow {
    pub context: Option<RelationHandle>,
    pub partition_by: Vec<Box<str>>,
    pub order_by: Vec<SortColumn>,
    pub require_unique_keys: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SeriesTransform {
    IsNull {
        invert: bool,
    },
    Fill {
        replacement: SeriesOperand,
    },
    Map {
        from: Vec<TabularScalar>,
        to: Vec<TabularScalar>,
        keep_unmatched: bool,
    },
    Trim,
    Lower,
    Upper,
    Replace {
        from: Box<str>,
        to: Box<str>,
    },
    Substring {
        start: i64,
        length: i64,
    },
    SplitPart {
        separator: Box<str>,
        part: i64,
    },
    DatePart {
        part: Box<str>,
    },
    DateTruncate {
        unit: Box<str>,
    },
    DateAdd {
        unit: Box<str>,
        amount: i64,
    },
    DateDifference {
        unit: Box<str>,
        other: SeriesOperand,
    },
    Clip {
        lower: f64,
        upper: f64,
    },
    Bin {
        edges: Vec<f64>,
        labels: Vec<Box<str>>,
    },
    Difference {
        order: usize,
        window: SeriesWindow,
    },
    PercentChange {
        periods: usize,
        window: SeriesWindow,
    },
    Shift {
        periods: usize,
        lead: bool,
        window: SeriesWindow,
    },
    Rolling {
        operation: WindowOperation,
        size: usize,
        min_periods: usize,
        window: SeriesWindow,
    },
    Cumulative {
        operation: WindowOperation,
        window: SeriesWindow,
    },
    Rank {
        dense: bool,
        descending: bool,
        nulls_first: bool,
        window: SeriesWindow,
    },
    FillDirection {
        forward: bool,
        window: SeriesWindow,
    },
    Standardize,
    InverseStandardize {
        mean: f64,
        standard_deviation: f64,
    },
    DummyInformation {
        base_level: Box<str>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeriesReduction {
    Length,
    Count,
    Sum,
    Mean,
    StandardizationStatistics,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PivotLevel {
    pub value: TabularScalar,
    pub name: Box<str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PivotAggregate {
    Sum,
    Mean,
    Min,
    Max,
    Count,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PivotSpec {
    pub keys: Vec<Box<str>>,
    pub category: Box<str>,
    pub value: Box<str>,
    pub levels: Vec<PivotLevel>,
    pub aggregate: PivotAggregate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnpivotSpec {
    pub keys: Vec<Box<str>>,
    pub columns: Vec<Box<str>>,
    pub variable_name: Box<str>,
    pub value_name: Box<str>,
    pub include_null: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResampleSpec {
    pub time: Box<str>,
    pub unit: Box<str>,
    pub keys: Vec<Box<str>>,
    pub columns: Vec<Box<str>>,
    pub aggregate: PivotAggregate,
}
