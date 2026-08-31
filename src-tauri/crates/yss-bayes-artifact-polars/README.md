# yss-bayes-artifact-polars

Polars-backed implementation of `yss-bayes-artifact-contract`.

This backend adapter is the only owner of Arrow IPC materialization, CSV export, posterior sample
paging, and trace/density/autocorrelation/posterior-predictive projections. It rejects malformed or
partial rows instead of silently dropping them and has no dependency on Tauri or Application state.
