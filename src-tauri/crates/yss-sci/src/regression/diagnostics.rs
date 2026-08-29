//! Regression diagnostics split by test family.
//!
//! The included files share one module scope to preserve private helper access
//! and the original `yss_sci::regression::diagnostics::*` API.

include!("diagnostics/breusch_pagan.rs");
include!("diagnostics/white.rs");
include!("diagnostics/im_test.rs");
include!("diagnostics/normality.rs");
include!("diagnostics/weighted.rs");
include!("diagnostics/reset.rs");
include!("diagnostics/vif.rs");
include!("diagnostics/leverage.rs");
