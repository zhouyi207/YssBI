# yss-sci

Statistical models and numerical algorithms over `yss-sci-linalg` matrices, vectors
and numeric slices. Matrix arithmetic, checked factorizations and rank conventions
use `yss-sci-linalg`; this crate does not depend on faer. Public computation contracts
use `yss-sci-contract`. The crate has no Arrow, Polars or chrono dependency.
Tabular input alignment and transformations belong to `yss-sci-runtime::data`.

`regression::fit` prepares numerical designs and projects model fits into neutral
results. `ts::models` prepares ADF/VAR/VEC computations. DID randomization inference
belongs to `regression::panel::did`, and kernel density estimation to `stats::density`.

`stats::linear_hypothesis` owns constraint parsing, linearization, parameter order,
matrix construction, test selection and `at()` interpretation. It uses
`yss-math-expr` for generic syntax and validated t/Wald inputs in `stats::linear_test`.
Project/result identity checks and report retrieval remain in Application.

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

The OLS numerical method uses SVD rank diagnostics and Cholesky of the cross
product. OLS, WLS, GLS, Prais and the IV estimators propagate rank computation
errors and reject rank-deficient designs or insufficient residual degrees of
freedom. Collinearity removal also propagates decomposition failure rather than
using it to choose columns to drop.

WLS, GLS and Prais compute total variation in the transformed space, centering
along the transformed intercept when one is configured. Their shared
`transformed_total_ss` preserves translation invariance with an intercept; without
one it returns the uncentered squared norm. Prais only reports a fitted result
when the AR coefficient change meets its tolerance. Exhausted iterations and
invalid iteration settings return errors. Finite AR estimates remain clipped to
`[-0.999, 0.999]` to retain a stationary transform; nonfinite estimates are errors.

Model APIs use `yss_sci_linalg::Mat` and `Col`; serialized results retain their existing
field and row/column meanings. Call sites use the shared OLS configuration. Structural migration does not imply that every model has
already adopted a new solver or convergence policy.
