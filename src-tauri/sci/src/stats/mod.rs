mod common;

pub mod t_test;
pub mod wald_test;

pub use common::Alternative;
pub use t_test::{t_test, TTestResult};
pub use wald_test::{wald_test, WaldTestResult};
