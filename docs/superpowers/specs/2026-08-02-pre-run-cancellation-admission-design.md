# Pre-run Cancellation Admission Design

## Goal

Ensure project replacement or clearing can cancel and drain graph execution while it is still loading function resources, before any resource from the old project is committed.

## Root cause

`ProjectState::execute_graph` currently creates its cancellation token before function loading but calls `ProjectRunRegistry::track_pre_run` only after function loading, compilation, resource snapshotting, and runtime setup. Project activation drains only registered preparing, active, and finalizing runs, so it cannot observe or cancel an execution blocked inside `load_function_resources`.

## Chosen design

Use the existing `ProjectRunRegistry` pre-run lifecycle as the single authority for the entire preparation phase.

Immediately after creating the cancellation token, `execute_graph` will:

1. Acquire the `project_store` read lock briefly.
2. Clone the current `ProjectRunRegistry` and `ProjectSessionId`.
3. Register the token with `track_pre_run` while the same read lock still protects that store/session pairing.
4. Release the lock before function loading or any other I/O or long-running work.

The returned `ProjectPreRunRegistration` remains alive through function loading, compilation, resource preparation, execution, and success finalization. Its existing RAII drop behavior removes the preparing registration on every early return or error.

The later duplicate `track_pre_run` call before `RunExecutor::run` is removed. The existing finalization gate continues to use the same registration.

## Concurrency contract

The registration and activation drain have only two valid orderings:

- Registration wins: activation obtains the same registry/session afterward, cancels the preparing token, and waits for the registration to drop before publishing the replacement project.
- Drain wins: `track_pre_run` observes the project as draining, cancels the token, and rejects execution before function loading begins.

No global lock is held during filesystem I/O, compilation, waiting, or execution. No second cancellation registry or generation-polling mechanism is introduced.

## Error and cleanup behavior

- Drain rejection remains represented by `ProjectRunRegistrationError::ProjectDraining` and is mapped to the existing string error boundary.
- Cancellation during function loading is detected by existing token checks and lifecycle commit checkpoints.
- Early compilation or setup failures drop the pre-run registration automatically, unblocking activation.
- Existing active-run registration, run-scoped cancellation, finalization protection, and result publication semantics remain unchanged.

## Testing

The existing production regressions are the required red/green proof:

- `project::production_tests::project_replacement_during_function_loading_cancels_before_old_resource_insert`
- `project::project_activation::tests::activation_and_pre_run_function_loading_complete_without_deadlock`

Broader focused verification will cover `ProjectRunRegistry`, project activation/lifecycle behavior, Rust checking and formatting, and `git diff --check`. The known-red complete Rust suite will not be rerun.

After independent review passes, update only the relevant rows in `TODO.md` under `## node_architecture 进度` and record evidence in a dedicated SDD ledger.
