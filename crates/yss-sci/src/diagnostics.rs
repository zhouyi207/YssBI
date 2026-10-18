//! Model diagnostics and serial-correlation tests.
pub mod breusch_pagan;
pub mod im_test;
pub mod leverage;
pub mod normality;
pub mod reset;
pub mod serial_correlation;
pub mod vif;
pub mod weighted;
pub mod white;

pub mod comparison;
pub mod design;
pub mod influence;
pub mod reclassification;
pub mod residual;
