//! Process control and capability own variation assumptions, not graph or chart rendering.
mod capability;
mod control;
mod gage;
mod process;
use crate::regression::models::common::{Result, failed, finite, parameter, validate};
pub use capability::process_capability;
pub use control::control_chart;
pub use gage::measurement_system;
use yss_sci_contract::{execution::ScientificExecutionControl as Control, quality::*};
