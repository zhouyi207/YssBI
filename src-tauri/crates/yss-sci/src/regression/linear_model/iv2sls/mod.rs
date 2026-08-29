//! IV:2SLS (Instrumental Variables Two-Stage Least Squares)
//!
//! Stata ivregress 2sls: depvar [varlist1] (varlist2 = varlistiv)
//! - varlist1: exogenous variables (in both stages)
//! - varlist2: endogenous variables (instrumented in stage 1)
//! - varlistiv: instruments (stage 1 only)
//!
//! Stage 1: Regress each endogenous on Z = [exog, instruments] → endog_hat
//! Stage 2: Regress Y on X = [exog, endog_hat] → β. VCE uses structural residuals u = y - X_struct*β.

mod critical_values;
mod first_stage;
mod fit;
mod types;

pub(crate) use first_stage::compute_first_stage_summary;
pub use types::*;
