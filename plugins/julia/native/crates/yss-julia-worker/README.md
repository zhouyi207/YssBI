# yss-julia-worker

Backend adapter for the reusable Julia worker process, embedded worker assets,
typed JSON-RPC failures, cancellation/restart lifecycle, and app-owned task
directories.

The crate depends on `yss-julia-runtime` for executable discovery and platform
command policy. It deliberately contains no Tauri, SCI, Polars, or Bayes domain
contracts; scientific adapters translate their own types at the boundary.

`cancel` targets the currently active task. `restart` terminates the worker process for
environment preparation or extension shutdown; plugin-wide fault recovery belongs to the host.
