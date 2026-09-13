# yss-sci-contract

Backend-neutral scientific contracts shared by application workflows, Execution,
the scientific runtime and numerical models.

- `scientific`: the OLS/ACF backend port, requests/results, admission controls and
  backend failures. These types contain no execution-plan or project identities.
- `regression`: the single OLS configuration/default and covariance selection.
  `fit` defines neutral computed model statistics; `report` defines typed OLS
  summary records. Report construction and labels belong to the runtime.
- `hypothesis`: neutral hypothesis requests, results, alternatives and errors.
- `panel`: DID inputs, inference results and typed unavailable/error codes.
- `density`: kernel-density input and output records.
- `computation`: observation metadata, missing-value policy and category roles.
- `error`: stable operation and scientific error vocabulary.

This crate owns data and callable contracts, not algorithms, report rendering,
project/database state, Tauri, Polars, faer or concrete backend implementations.
Observation metadata records row selection counts and the applied missing-value
policy. There is no global approximate-equality tolerance configuration.

Bayesian plugin inputs and cancellation controls belong to the plugin's
`yss-bayes-worker`; plugins do not depend on this crate.
