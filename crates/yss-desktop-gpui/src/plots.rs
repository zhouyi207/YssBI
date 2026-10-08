//! Shared native chart presentation. Application owns every value and statistic.
mod axes;
pub(crate) use axes::axis_value;
pub(crate) mod cartesian;
pub(crate) mod histogram;
