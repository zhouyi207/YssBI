//! VEC implementation split by workflow.
//!
//! Included files share one module scope so the public `ts::vec::*` API remains unchanged.

use super::var::cholesky_lower_in_place;

include!("vec/types.rs");
include!("vec/stage.rs");
include!("vec/estimate.rs");
include!("vec/vecrank.rs");
include!("vec/stats.rs");

include!("vec/postestimation.rs");
