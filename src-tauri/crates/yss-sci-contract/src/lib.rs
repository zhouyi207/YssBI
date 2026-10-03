//! Backend-neutral contracts grouped by scientific domain.

pub mod anova;

pub mod association;
pub mod causal;
pub mod density;
pub mod descriptive;
pub mod diagnostics;
pub mod distribution;
pub mod execution;
pub mod hypothesis;
pub mod inference;
pub mod longitudinal;
pub mod meta;
pub mod multivariate;
pub mod panel;
pub mod regression;
pub mod spatial;
pub mod survival;
pub mod time_series;
pub mod visualization;

mod error;
mod observation;

pub use error::{SciError, SciOperationCode};
pub use observation::{CategoricalRole, MissingValuePolicy, StatisticalObservationMetadata};
pub mod decision;
pub mod doe;
pub mod psychometrics;
pub mod quality;

pub mod path;
pub mod power;
pub mod survey;
