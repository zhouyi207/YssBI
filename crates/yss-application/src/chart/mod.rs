//! Chart resources and database-backed presentation, independent of graph execution.

mod projection;
mod query;
mod resources;

pub use projection::{ChartPlotResult, PlotAxisFormat, PlotPoint};
pub use query::{ChartPlotApplicationError, ChartPlotQuery};
pub use resources::ChartApplicationError;
