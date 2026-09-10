# Julia Bayes worker protocol

The Julia worker is a separate, restartable process owned by Rust. It is the production adapter for Bayesian inference only; it does not own project state, write project DuckDB files, or implement ACF/serial/hypothesis operations.

## Current operation

`worker.jl` registers exactly one scientific operation:

| Operation   | Handler                           | Purpose                                                                      |
| ----------- | --------------------------------- | ---------------------------------------------------------------------------- |
| `bayes_fit` | `ops/bayes_fit.jl::run_bayes_fit` | Fit supported Turing regression models and publish typed inference artifacts |

Bayes helpers live under `ops/bayes/`:

- `expression.jl`
- `runtime.jl`
- `turing_generic_normal.jl`

The current Turing adapter supports the model families accepted by `bayes_fit` implementation (Normal, BernoulliLogit and PoissonLog regression with supported scalar priors). Rust validates the application model and compiles predictor/likelihood kernels before dispatch.

## Control plane

The worker uses newline-delimited JSON-RPC 2.0 over stdin/stdout. Stdout is protocol-only; diagnostics go to stderr.

Methods:

- `ping`: readiness check.
- `run`: execute `bayes_fit` with `taskId`, operation, paths and typed parameters.
- `cancel`: cooperative cancellation notification for the active task.
- `progress`: worker-to-host notification carrying `taskId`, stage and optional completed/total values.

Rust serializes compute requests through one reusable worker process. Cancellation may restart the process if cooperative cancellation cannot complete safely.

## Typed errors

Julia responses carry an explicit error `code`; Rust maps that code to `JuliaWorkerErrorCode` and then to a stable Bayes application code. Current worker-level protocol categories include:

- `invalid_request`
- `invalid_parameters`
- `unsupported_capability`
- `sampling_failed`
- `package_unavailable`
- `cancelled`
- `internal_error`

The Rust host adds typed lifecycle/storage categories such as runtime unavailable, environment unavailable, asset update failure, task-directory failure, input write failure, timeout and invalid response.

`worker_protocol.jl` owns the typed `WorkerTaskError` classification. Unsupported model capabilities and sampling failures are converted at their source; `TaskCancelled` is propagated without being relabeled as a sampling failure. Safe structured details are limited to `column`, `row`, `parameter` and `path`. `JuliaWorkerError::Display` exposes the stable code; diagnostic text remains private and is never parsed to infer a category.

## App-owned task root

Every task directory is owned by `JuliaWorkerTaskDirectory` and must be the exact canonical direct child:

```text
<app-data>/julia-worker/tasks/<task-id>/
```

Ownership checks include:

- task IDs are 1–128 ASCII alphanumeric/`-`/`_` characters;
- canonical tasks root must equal the app-owned expected root;
- task path must equal `tasks_root/<task-id>` and have that exact parent;
- cleanup revalidates canonical app root, tasks root and task path before `remove_dir_all`.

The RAII owner cleans the directory on drop. If an inference result retains posterior samples or posterior predictive artifacts, `JuliaBayesBackend` transfers the task-directory owner to the result; clearing/dropping the result then releases the artifacts. A result that claims retained artifacts without an owner is rejected.

## Data plane

Rust writes exchange files inside the owned task directory:

- `input.arrow`: selected project data columns as Arrow IPC.
- `model_spec.json`: typed `BayesModelSpec`.
- `inference_config.json`: sampler configuration.
- `predictor_kernel.jl`: Rust-generated predictor kernel.
- `likelihood_kernel.jl`: Rust-generated likelihood kernel.
- `exchange_manifest.json`: versioned paths, input shape and predictor columns.

Julia writes:

- `output.arrow`: posterior samples when produced.
- `summary.json`: summaries and diagnostics.
- `metadata.json`: typed `InferenceResult` including `artifactManifest`.
- posterior predictive artifacts when requested by the model/run.

Rust validates that `artifactManifest.taskId` matches the worker task before accepting the result.

## Worker assets and environment

Rust embeds and publishes the worker environment beneath `<app-data>/julia-worker/`:

- `Project.toml`
- `Manifest.toml`
- `worker.jl`
- `worker_protocol.jl`
- `scientific_runtime.jl`
- current Bayes operation files

Each changed asset is written to an exclusive sibling temp file, flushed with `sync_all`, and atomically replaced. A failed publish cleans its temp file and returns a typed `AssetUpdateFailed` error; callers never intentionally observe a partially written asset.

The runtime probe accepts Julia `>=1.10,<2.0`, matching `Project.toml`; incompatible PATH candidates are skipped while later candidates are still checked. `Pkg.instantiate()` prepares pinned Julia dependencies, including Arrow/JSON3 and the Turing runtime. This step may download packages/artifacts and can fail independently of worker process startup.

## Interface rule

Business callers enter through `application::bayes` and `sci::api::bayes::BayesBackend`. They do not invoke worker operations or task paths directly. This adapter seam keeps process lifecycle, exchange files, typed error mapping and artifact ownership local to the Julia implementation.
