# yss-sci

Statistical models and numerical algorithms over native faer matrices/vectors
and numeric slices. Matrix arithmetic uses faer; checked factorizations and rank
conventions use `yss-linalg`. Public computation contracts use `yss-sci-contract`. The crate has no Polars or chrono dependency. Tabular input
alignment and transformations belong to `yss-sci-runtime::data`.

## OLS model boundary

`regression::linear_model::ols` owns the model, its fitted result and its errors:

- `mod.rs`: `OLS`, `OlsFit` and `OlsFitError`.
- `fit.rs`: least-squares numerical work and the intermediate solution.
- `inference.rs`: covariance, degrees of freedom, tests and confidence intervals.

`OLS.config` uses the shared `yss_sci_contract::regression::OlsOptions`. Node
defaults and the runtime use the same definition. Named covariance inputs from
other estimators are parsed into that configuration; invalid combinations fail
explicitly, and unimplemented covariance modes retain their diagnostic failure.

`OlsFit` is the numerical authority for coefficients, fitted values, residuals,
rank and inference statistics. It does not carry a second copy of coefficients
inside a nested model. Runtime reports project these computed values.

The OLS numerical method and rank threshold remain unchanged: SVD rank diagnostics
and Cholesky of the cross product. Model-level rank handling, rank-failure
propagation and iteration-stop semantics remain separate numerical follow-up
work, tracked in the repository's tolerance analysis and TODO.

Model APIs use `faer::Mat` and `Col`; serialized results retain their existing
field and row/column meanings. Call sites use the shared OLS configuration. Structural migration does not imply that every model has
already adopted a new solver or convergence policy.
