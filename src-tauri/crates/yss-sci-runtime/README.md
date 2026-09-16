# yss-sci-runtime

Stateless scientific computation entry points over `yss-sci`, called directly
by `yss-node-kernel`, `yss-graph-execution` result analyses and its focused SCI benchmark,
and `yss-application::ipc::commands`.

Runtime calls SCI with ordinary vectors, slices and contract records. It has no
faer or `yss-sci-linalg` dependency and does not construct numerical matrices or own
estimators. Arrow remains the tabular exchange representation; SCI converts
numeric inputs into Linalg matrices and returns computed results.

`ols` and `acf_pacf` accept neutral requests and `ScientificExecutionControl` from
`yss-sci-contract`. The OLS kernel forwards the execution cancellation and deadline;
result analyses and standalone IPC ACF/PACF use the existing 60-second deadline.
Application declares this dependency for its IPC commands. Desktop composition
and other application modules do not call it or construct/inject a backend object.
The runtime has no Execution dependency.
Cancellation and deadlines are checked before dispatch and after computation;
these checks do not promise cooperative interruption of a running matrix decomposition.

## Capability modules

| Module               | Responsibility                                                        |
| -------------------- | --------------------------------------------------------------------- |
| `computation`        | Implementation of the exported OLS/ACF functions and admission checks |
| `regression`         | SCI fit entry points and result serialization                         |
| `regression::report` | Report labels, fields and serialization                               |
| `regression::types`  | Existing report/model records used by capability APIs                 |
| `hypothesis`         | Neutral hypothesis and margins `at()` entry points into SCI           |
| `time_series`        | ACF/PACF, serial tests, ADF, VAR and VEC entry points                 |
| `panel`              | Entry point into SCI DID randomization inference                      |
| `data`               | Arrow panel/time alignment and tabular transformations                |
| `density`            | Density computation entry point                                       |

There is no empty `SciContext` or parallel `api/backends/rust` route. Capability
entry points call the corresponding SCI owner; report encoding stays here.

## OLS data flow

`OlsRequest.options` and the algorithm's configuration use the same
`yss_sci_contract::regression::OlsOptions`. The Graph catalog reads its OLS defaults
from that contract. The runtime passes the selected options to the model without
an intermediate configuration mirror.

SCI's `regression::fit::fit_ols` projects `OlsFit`, including its already-computed fitted
values and residuals. `regression::report::ols_report` returns typed model and
coefficient statistics without duplicating observation arrays. The OLS function
returns these statistics alongside ordinary fitted/residual vectors and the fitted
design columns. Execution shares that immutable result across its report outputs;
Application projects report references, validates the session and retained result,
and asks Execution to derive ACF/PACF, serial-test or hypothesis inputs from the same
fit. Execution calls the corresponding runtime functions. Serial-test input/output
records belong to `yss-sci-contract::serial_tests`. No opaque JSON report crosses
this boundary.

Tabular preparation accepts Arrow arrays and `RecordBatch` values. Time alignment preserves
Int64/Date32 and column metadata, fills gaps with nulls, and checks duplicate/null times and
the bounded output size before allocating a complete grid. Lag, difference, percentage change,
and rolling means retain their numeric/null policies. Panel batch adapters reuse the existing
`align_panel` and `panel_diff` routines; they preserve entity/date types at the Arrow boundary.
These routines prepare already admitted inputs and do not own relational plans or project data.

The crate does not own project/database state, graph scheduling, result storage,
Tauri commands, frontend state, Julia processes or Bayesian worker lifecycle.
