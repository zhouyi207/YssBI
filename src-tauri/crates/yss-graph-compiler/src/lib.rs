//! Neutral Graph document lowering and its immutable package contract.
//!
//! This crate validates the captured document revision and lowers Graph-owned
//! values without depending on Project authority, Application orchestration,
//! or Execution's package model.

#![deny(unused_must_use)]

mod compiler;
mod error;
mod package;

pub use compiler::{GraphCompilationInput, compile};
pub use error::{GraphCompileError, GraphCompileErrorCode};
pub use package::{
    GraphCompiledPackage, GraphInputBinding, GraphInputKind, GraphInputSource,
    GraphObservationIntent, GraphOperation, GraphParameterHandle, GraphParameterPayload,
    GraphParameterScalar, GraphParameterValue, GraphSourceIdentity, GraphValueRef,
};
