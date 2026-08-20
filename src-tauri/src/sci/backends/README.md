# SCI backend adapters

This directory contains concrete adapters behind the application SCI interface.

## Current matrix

```text
backends/
├─ rust/
│  ├─ stats/hypothesis.rs
│  └─ time_series/{acf_pacf,serial_tests}.rs
└─ julia/
   └─ bayes/
```

| Capability | Adapter | Status |
|---|---|---|
| ACF/PACF | Rust | Production |
| Serial-correlation tests | Rust | Production |
| t/Wald hypothesis tests | Rust | Production |
| Bayesian inference | Julia | Production through `BayesBackend` |

## Seam rules

- Callers enter through `crate::sci::api`, not these implementation paths.
- Rust adapters translate `yss-sci` values/errors into application-owned typed models.
- Julia is a real adapter only for the `BayesBackend` interface.
- Time-series and hypothesis interfaces dispatch directly to Rust; there is no hypothetical Julia adapter to keep in sync.
- Adapters do not own ProjectState, DuckDB, Tauri transport or frontend state.
- Stable error classification occurs at the adapter seam; diagnostic prose is never parsed as a protocol.

This shape preserves depth: consumers learn one typed interface, while backend-specific conversion, runtime lifecycle and error mapping remain local to their adapter implementation.
