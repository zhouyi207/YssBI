//! Shared native chart presentation. Application owns every value and statistic.
mod axes;
pub(crate) use axes::axis_value;
pub(crate) mod cartesian;
pub(crate) mod composite;
pub(crate) mod correlogram;
pub(crate) mod distribution;
mod frame;
pub(crate) mod histogram;
pub(crate) mod interval;
pub(crate) mod matrix;
pub(crate) mod nomogram;
pub(crate) mod wordcloud;
