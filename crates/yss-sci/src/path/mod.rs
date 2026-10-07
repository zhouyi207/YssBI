//! Observed continuous-variable interactions and path estimators.
use crate::regression::models::common::{Result, finite, parameter, validate};
use yss_sci_contract::{execution::ScientificExecutionControl as Control, path::*};
mod bootstrap;
mod effects;
mod mediation;
mod moderation;
mod preparation;
mod recursive;
pub use mediation::mediation;
pub use moderation::moderation;
pub use recursive::{parse_equations, recursive_path};
