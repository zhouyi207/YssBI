# Julia Worker Protocol

The Julia worker is a separate, restartable process. Rust owns its lifecycle;
Julia never owns project state or writes project DuckDB files.

## Resources and environment

- `Project.toml` and `Manifest.toml` pin `Arrow.jl` and `JSON3.jl`.
- `worker.jl` owns JSON-RPC protocol handling and dispatch.
- `ops/*.jl` owns individual scientific operations, starting with
  `ops/acf_pacf.jl`.
- Rust embeds these files, then writes them to `<app-data>/julia-worker/`
  when needed.
- Rust can prepare the worker environment by running `Pkg.instantiate()`. This
  may download packages and artifacts, so user-facing flows should surface clear
  status and error messages when a Julia backend is first enabled.

## Transport

- **Control plane:** newline-delimited JSON-RPC 2.0 over worker stdin/stdout.
  Stdout contains protocol messages only; diagnostics use stderr.
- **Data plane:** a per-task directory beneath
  `<app-data>/julia-worker/tasks/<task-id>/`.
  - `input.arrow`: Rust → Julia Arrow IPC table.
  - `output.arrow`: Julia → Rust Arrow IPC table.
  - `metadata.json`: JSON result metadata.

The worker accepts a `cancel` notification. Cancellation is cooperative; the
Rust host may restart an unresponsive worker as the final fallback.

## ACF/PACF operation

The Rust `src/sci` facade writes a single `value: Float64` Arrow input column
and asks the worker to run `acf_pacf`. The output table contains `lag`, `acf`,
and nullable `pacf` (lag zero has no PACF). Business callers should enter via
`command_sci` / `src/sci`, not by invoking the worker directly.

Correctness validation lives in Rust tests. The ACF/PACF golden tests run the
Rust backend and, when `YSSBI_RUN_JULIA_TESTS=1` is set, the Julia worker backend
against fixed expected results. Frontend code should call business commands such
as `compute_acf_pacf`; it should not invoke Julia operations directly.
