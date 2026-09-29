pub mod categorical;
pub mod linear_hypothesis;
mod linear_test;
pub mod nonparametric;
pub mod sample_mean;
pub mod t_test;
pub mod variance;
pub mod wald_test;

pub use t_test::t_test;
pub use wald_test::wald_test;
