# yss-sci-runtime

Application-facing scientific computation entry points over `yss-sci`.

The composition root constructs `SciRuntimeBackend`, which directly implements
`yss_sci_contract::scientific::ScientificBackend`. Application and Execution share
the neutral OLS/ACF contracts; the implementation has no Execution dependency.
Cancellation and deadlines are checked at the synchronous backend admission
boundary. These checks do not promise cooperative interruption of a running
matrix decomposition.

## Capability modules

| Module               | Responsibility                                                  |
| -------------------- | --------------------------------------------------------------- |
| `service`            | Private implementation of the shared scientific backend port    |
| `regression`         | Model input preparation and projection of computed statistics   |
| `regression::report` | Report labels, fields and serialization                         |
| `regression::types`  | Existing report/model records used by capability APIs           |
| `hypothesis`         | Validated t/Wald-test entry points over shared contract results |
| `time_series`        | ACF/PACF, serial tests, ADF, VAR and VEC entry points           |
| `panel`              | Panel inference workflows                                       |
| `data`               | Polars-based panel/time alignment and tabular transformations   |
| `density`            | Density computation entry point                                 |

There is no empty `SciContext` or parallel `api/backends/rust` route. Each
capability owns its entry and implementation together.

## OLS data flow

`OlsRequest.options` and the algorithm's configuration use the same
`yss_sci_contract::regression::OlsOptions`. The Graph catalog reads its OLS defaults
from that contract. The runtime passes the selected options to the model without
an intermediate configuration mirror.

`regression::fit_ols` projects `OlsFit`, including its already-computed fitted
values and residuals. `regression::report::ols_report` returns the typed contract
summary consumed by Execution. Conversion into runtime values happens at that
output boundary; the scientific port does not return an opaque JSON report.
Existing report field names and statistical calculations are preserved.

The crate does not own project/database state, graph scheduling, result storage,
Tauri commands, frontend state, Julia processes or Bayesian worker lifecycle.
