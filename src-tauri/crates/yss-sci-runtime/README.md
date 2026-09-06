# yss-sci-runtime

Application-facing, synchronous scientific-computing runtime over `yss-sci`.

This crate owns the typed density, regression, panel, time-series, and hypothesis interfaces;
their report models; and the Rust adapters that map algorithm results into `yss-sci-contract`
errors. The modules remain together because the API, models, and adapters share one execution
contract and evolve as a unit.

`api::node_statistics::fit_ols` accepts explicit intercept and covariance configuration, reusing
the existing OLS implementation and regression report model. Coefficient labels account for
whether the fitted design includes an intercept.

The crate does not own project/database state, graph execution, Tauri commands, frontend wire
errors, Julia processes, or Bayesian worker lifecycle. Those responsibilities remain in their
dedicated crates or in Application and the composition root.
