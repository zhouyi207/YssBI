# SCI application module

`src-tauri/src/sci/` 是尚待迁移的主应用科学计算 runtime interface，不是独立算法或 Bayes model crate。当前调用方向是：

```text
Bayes commands / application
  → yss-bayes-model draft/parser/validated spec
  → yss-bayes-result diagnostics/task/result projections
  → yss-bayes-worker validated task/opaque handle/port client
  → yss-bayes-worker-julia::JuliaBayesWorkerAdapter

other scientific application / node_system kernels
  → sci::api remaining runtime interface
  → sci::backends adapter
  → yss-sci Rust algorithms
```

`yss-bayes-model` 是 Pure Leaf，唯一拥有 Bayes draft、表达式解析、校验与不可变 spec 构造；`yss-bayes-result` 唯一拥有 diagnostics、task/result projections、artifact manifest 与 plot/page DTO；`yss-bayes-worker` 唯一拥有 validated task、opaque handle、worker port/client 与临时构造 capability。commands、Application 和 Julia adapter 直接依赖对应 crate，不经过根 facade。根 `sci::api` 不再保留 test-only Bayes backend 或 Polars input-validation 镜像；生产和测试都通过同一个 `BayesWorkerPort` seam。

## Ownership

本 module 拥有：

- application-facing scientific request/result types；
- ACF/PACF、serial tests、hypothesis 和 node statistics interface；
- typed regression models/statistics；
- Rust SCI adapters；

Bayes model/draft/parser/validation 由 `yss-bayes-model` 拥有；Bayes diagnostics、task/result
projection 与 artifact manifest 由 `yss-bayes-result` 拥有；shared statistical input/settings/control
与 stable `SciError` 由 `yss-sci-contract` 拥有；validated worker task、opaque handle、
`BayesWorkerPort` 与 `BayesWorkerClient` 由 `yss-bayes-worker` 拥有；Julia 的具体 port
实现由 `yss-bayes-worker-julia` Backend Adapter 拥有。

本 module 不拥有：

- ProjectState 或 resource authority；
- DuckDB/database declarations；
- DataView editing、EditHistory、undo/redo 或 export；
- Tauri transport/event presentation；
- frontend state。

这些职责分别位于 `project/`、`database/`、`commands/` 和 React modules。

## Module map

```text
sci/
├─ api/
│  ├─ density.rs
│  ├─ node_statistics.rs
│  ├─ stats/hypothesis.rs
│  └─ time_series/{acf_pacf,serial_tests}.rs
├─ backends/
│  └─ rust/
├─ models/{regression,panel_did}.rs
└─ engine.rs
```

The concrete Julia worker adapter is composition-side in `yss-bayes-worker-julia`; it implements
the contract-owned `yss-bayes-worker::BayesWorkerPort` and is not a SCI backend module.

## Production execution

| Capability | Production adapter |
|---|---|
| ACF/PACF | Rust |
| Durbin-Watson, Ljung-Box, Breusch-Godfrey | Rust |
| t/Wald hypothesis tests | Rust |
| Node regression/time-series statistics | Rust over `yss-sci` |
| Bayesian inference | `yss-bayes-worker-julia::JuliaBayesWorkerAdapter` through `BayesWorkerPort` |

ACF/PACF 与 context-free KDE 直接位于 SCI API；`SciContext::rust()` 仍为 serial/hypothesis
显式选择当前 Rust path。不存在 Julia time-series adapter；Julia 只在
`BayesWorkerPort` seam 作为真实 adapter。参见 [backends](./backends/README.md) 与
[Julia worker protocol](../../julia/README.md)。

## Typed regression statistics

`api::node_statistics::RegressionFit` carries typed `RegressionStatistics` rather than treating report JSON as authority:

- `Linear`
- `Binary { link }`
- `Prais`

Each variant combines `RegressionCoefficientStatistics`—including the authoritative coefficient covariance matrix—with its typed model statistics. Observation filtering/tolerance facts are captured by `StatisticalObservationMetadata`. `regression_report` explicitly projects `betas` and `cov_beta` for hypothesis testing; binary reports also project typed likelihood/model statistics.

## Error contract

- Rust adapters map algorithm validation failures to `SciError` stable codes.
- `ValidationReport.ok` is derived from its private error set; draft-to-spec conversion fails closed without production `expect` branches.
- Julia worker errors are classified by typed `JuliaWorkerErrorCode`, not by parsing diagnostic prose.
- `JuliaBayesWorkerAdapter` returns typed `BayesWorkerError`; Application maps it to stable Bayes task codes without copying backend prose.
- Commands convert these errors to the project-wide `{ code, details, incidentId }` wire; backend prose stays diagnostic-only.
