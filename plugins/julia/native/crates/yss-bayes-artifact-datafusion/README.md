# yss-bayes-artifact-datafusion

DataFusion-backed implementation of `yss-bayes-artifact-contract`.

This plugin backend adapter owns Arrow IPC queries, CSV export, posterior sample paging, and
trace/density/autocorrelation/posterior-predictive projections. Native DataFusion plans project,
filter and page immutable Julia artifact files. Queries share one memory pool and stream Arrow
batches; CSV export does not collect a whole table. A single scan partition retains file row order.

Validation scans all required columns before paging, including rows outside a selected parameter
or page. Negative identities, null/non-finite values, empty predictive files and inconsistent
response transforms fail explicitly. Plot inputs have a separate memory budget. KDE and
autocorrelation are computed inside this plugin while retaining their existing numerical rules.

The reader uses a private Tokio runtime with a deadline and serial admission, and handles callers
already inside Tokio without nesting runtimes. It uses Arrow IPC directly and has no dependency on
host database models, scientific runtimes, Tauri, Project or Application.
