# yss-bayes-worker-julia

Concrete Julia backend adapter for `yss-bayes-worker`.

This crate owns the `JuliaBayesWorkerAdapter`, generation of Julia predictor and likelihood
kernels, typed Julia exchange files, worker-task state, cancellation delivery, and safe
materialization of worker results and artifacts. It implements `BayesWorkerPort` by composing the
backend-neutral Bayes contracts with `yss-julia-worker`.

The crate does not own the reusable Julia process, application task storage, Tauri commands,
transport DTOs, or project/database authority. Those responsibilities remain in
`yss-julia-worker`, Application, and the composition root.
