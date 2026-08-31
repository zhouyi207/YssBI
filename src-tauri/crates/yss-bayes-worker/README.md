# yss-bayes-worker

Owns the backend-neutral Bayesian worker boundary: validated tasks, opaque task and artifact
handles, terminal/error contracts, and the client-to-worker port.

`BayesWorkerClient` grants an unforgeable `BayesWorkerAuthority` only while invoking a port.
Backend adapters use that temporary capability to create handles and results; application and
transport code can only consume the validated projections.

This crate does not own Julia process management, filesystem artifacts, Polars dataframes, Tauri
commands, or application task storage.

The production Julia implementation lives in the separate `yss-bayes-worker-julia` Backend
Adapter.
