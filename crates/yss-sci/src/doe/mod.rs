//! Designed experiments: factor-level response summaries and design diagnostics.
mod design;
mod dose;
mod range;
mod surface;
mod uniform;
use crate::regression::models::common::{Result, finite, parameter, validate};
pub use design::{design_dimensions, generate_design};
pub use dose::dose_response;
pub use range::range_analysis;
pub use surface::{response_surface, response_surface_parameter_count};
use yss_sci_contract::{doe::*, execution::ScientificExecutionControl as Control};
