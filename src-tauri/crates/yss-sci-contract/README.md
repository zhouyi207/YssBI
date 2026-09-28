# yss-sci-contract

> Status: Current
> Scope: 按 SCI 领域组织的中立输入、选项、结果及共享执行控制
> Canonical owners: 本 crate 源码拥有科学计算契约；算法与报告组装由 SCI 和 Runtime 拥有
> Update when: 中立契约、领域归属或执行控制改变时

Backend-neutral scientific contracts shared by application workflows, Execution,
the scientific runtime and numerical models.

Domain names follow [SCI's category mapping](../yss-sci/README.md#domain-organization).
Cross-domain execution and observation contracts remain shared.

- `execution`: `ScientificExecutionControl`, cancellation and computation errors.
  These types contain no execution-plan or project identities, and define no backend trait.
- `regression`: the single OLS configuration/default and covariance selection.
  `linear` defines OLS/WLS/GLS computation requests/results; `fit` defines neutral
  regression statistics; `report` defines typed OLS
  summary records. Report construction and labels belong to the runtime.
  `summary` owns the selected linear-summary contents and analysis defaults; it
  does not contain UI layout, graph identity, or cache state.
- `hypothesis`: neutral hypothesis requests, results, alternatives and errors.
- `time_series::acf_pacf`: ACF/PACF requests and results.
- `diagnostics::serial_correlation`: serial-correlation requests and BG/Q/Durbin-Watson results.
- `panel`: numerical panel-model fits.
- `causal::iv`: IV estimator selection and computed fits.
- `causal::did`: DID inputs, inference results and typed unavailable/error codes.
- `density`: kernel-density input and output records.
- `distribution`: probability distribution parameters and typed samples.
- `observation`: observation metadata, missing-value policy and category roles,
  exported from the crate root for all domains.
- `error`: stable operation and scientific error vocabulary.

This crate owns data and execution-control contracts, not algorithms, report rendering,
project/database state, Tauri, Polars, faer or concrete backend implementations.
Observation metadata records row selection counts and the applied missing-value
policy. There is no global approximate-equality tolerance configuration.

Bayesian plugin inputs and cancellation controls belong to the plugin's
`yss-bayes-worker`; plugins do not depend on this crate.

`ScientificExecutionControl::check` gives cancellation priority over deadline expiry.
ACF/PACF checks during input scans, ACF accumulation and PACF recursion; linear
regression checks between stages, without interrupting a running decomposition.
Scheduling, concurrency and the budget remain caller-owned.
