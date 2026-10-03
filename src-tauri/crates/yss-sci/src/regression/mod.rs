//! Regression estimators and shared design/covariance calculations.
pub mod collinearity;
pub mod covariance;
pub(crate) mod design;
pub mod discrete;
pub mod linear;
pub mod models;

pub mod postestimation;
