# yss-bayes-result

Pure Bayesian inference diagnostics, task projections, artifact manifests, and
plot/page result DTOs shared by workers, application workflows, adapters, and
transport.

This crate owns no model construction, worker/process capability, filesystem
lease, dataset, Polars frame, Julia protocol, application state, or Tauri API.
Artifact lifetime remains with the application workflow that materializes the
worker output.
