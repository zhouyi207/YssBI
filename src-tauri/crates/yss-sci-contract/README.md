# yss-sci-contract

Backend-neutral scientific contracts shared by application workflows, Execution,
the scientific runtime and numerical models.

- `scientific`: the OLS/ACF backend port, requests/results, admission controls and
  backend failures. These types contain no execution-plan or project identities.
- `regression`: the single OLS configuration/default and covariance selection.
  The report submodule defines typed OLS summary records; construction and labels
  belong to the runtime.
- `hypothesis`: shared alternatives and computed t/Wald results.
- `computation`: validated statistical inputs and observation metadata.
- `control`: monotonic execution/cancellation controls used by scientific workers.
- `error`: stable operation and scientific error vocabulary.

This crate owns data and callable contracts, not algorithms, report rendering,
project/database state, Tauri, Polars, faer or concrete backend implementations.
Observation metadata records row selection counts and the applied missing-value
policy. There is no global approximate-equality tolerance configuration.
