# yss-bayes-artifact-contract

Backend-neutral port for reading and exporting Bayesian result artifacts.

This crate owns only the artifact-reader capability and its typed failure categories. It depends on
the canonical `yss-bayes-result` projections and deliberately has no knowledge of Polars, Arrow IPC,
Tauri, Application workflows, or a concrete filesystem implementation.
