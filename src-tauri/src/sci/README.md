# SCI application module

`src-tauri/src/sci/` 是主应用科学计算 interface，不是独立算法 crate。它位于 command/application/node runtime 与具体计算 implementation 之间：

```text
commands / application / node_system kernels
  → sci::api typed interface
  → sci::backends adapter
  → yss-sci Rust algorithms or Julia Bayes worker
```

这个 seam 集中输入规范化、typed output、backend dispatch 与 error mapping，避免调用方直接依赖 `yss_sci` 深层路径或 Julia protocol。较小 interface 隐藏 backend 细节，提供 depth、leverage 和 locality。

## Ownership

本 module 拥有：

- application-facing scientific request/result types；
- ACF/PACF、serial tests、hypothesis 和 node statistics interface；
- typed regression models/statistics；
- Bayes model/draft/validation/result 与 `BayesBackend` interface；
- Rust/Julia adapters；
- stable `SciError` mapping。

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
│  ├─ bayes/
│  ├─ density.rs
│  ├─ node_statistics.rs
│  ├─ stats/hypothesis.rs
│  └─ time_series/{acf_pacf,serial_tests}.rs
├─ backends/
│  ├─ rust/
│  └─ julia/bayes/
├─ models/{regression,panel_did}.rs
├─ engine.rs
└─ error.rs
```

## Production execution

| Capability | Production adapter |
|---|---|
| ACF/PACF | Rust |
| Durbin-Watson, Ljung-Box, Breusch-Godfrey | Rust |
| t/Wald hypothesis tests | Rust |
| Node regression/time-series statistics | Rust over `yss-sci` |
| Bayesian inference | `JuliaBayesBackend` |

ACF/PACF 与 context-free KDE 直接位于 SCI API；`SciContext::rust()` 仍为 serial/hypothesis
显式选择当前 Rust path。不存在 Julia time-series adapter；Julia 只在 `BayesBackend` seam
作为真实 adapter。参见 [backends](./backends/README.md) 与
[Julia worker protocol](../../julia/README.md)。

## Typed regression statistics

`api::node_statistics::RegressionFit` carries typed `RegressionStatistics` rather than treating report JSON as authority:

- `Linear`
- `Binary { link }`
- `Prais`

Each variant combines `RegressionCoefficientStatistics`—including the authoritative coefficient covariance matrix—with its typed model statistics. Observation filtering/tolerance facts are captured by `StatisticalObservationMetadata`. `regression_report` explicitly projects `betas` and `cov_beta` for hypothesis testing; binary reports also project typed likelihood/model statistics.

## Error contract

- Rust adapters map algorithm validation failures to `SciError` stable codes.
- Julia worker errors are classified by typed `JuliaWorkerErrorCode`, not by parsing diagnostic prose.
- `JuliaBayesBackend` maps worker categories to stable Bayes codes and copies only safe structured details.
- Commands convert these errors to the project-wide `{ code, details, incidentId }` wire; backend prose stays diagnostic-only.
