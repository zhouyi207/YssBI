mod common;

pub mod t_test;
pub mod wald_test;

pub use common::Alternative;
pub use t_test::{TTestResult, t_test};
pub use wald_test::{WaldTestResult, wald_test};
