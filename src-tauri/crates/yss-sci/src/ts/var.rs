//! VAR implementation split by workflow.
//!
//! Included files share one module scope so the public `ts::var::*` API remains unchanged.

include!("var/types.rs");
include!("var/varsoc.rs");
include!("var/stata.rs");
include!("var/estimate.rs");
