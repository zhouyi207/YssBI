//! Panel Random Effects estimators split by workflow.
//!
//! The included files share one module scope to preserve private helper access
//! while keeping the public `fit_panel_re_*` API unchanged.

include!("re/shared.rs");
include!("re/be.rs");
include!("re/fgls.rs");
include!("re/mle.rs");
include!("re/twoway.rs");
include!("re/time.rs");
