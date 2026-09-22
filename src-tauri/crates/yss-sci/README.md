# yss-sci

Statistical models and numerical algorithms over `yss-sci-linalg` matrices, vectors
and numeric slices. Matrix arithmetic, checked factorizations and rank conventions
use `yss-sci-linalg`; this crate does not depend on faer. Public computation contracts
use `yss-sci-contract`. The crate has no Arrow, Polars or chrono dependency.
Tabular input alignment and transformations belong to `yss-sci-runtime::data`.

`regression::fit` prepares numerical designs and projects model fits into neutral
results. `ts::models` prepares ADF/VAR/VEC computations. DID randomization inference
belongs to `regression::panel::did`, and kernel density estimation to `stats::density`.

Residual normality tests are owned by `regression::diagnostics::normality`;
Durbin-Watson and other serial correlation tests by `ts::serial_correlation`.

Panel first differences take entity IDs and original time values; they do not
require a second time-ID vector. First-stage IV summaries derive dimensions from
their matrices and receive covariance/estimator choices through `FirstStageOptions`.

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

OLS and WLS share the overall coefficient test: nonrobust covariance uses the
classical mean-square ratio, while other covariance selections use a Wald test
excluding the intercept. A singular covariance or invalid Wald statistic returns
an inference error instead of a fabricated zero statistic and unit p-value.
WLS uses the shared typed `OlsCovariance` selection. Named covariance callers
are validated by `OlsOptions::from_covariance_parts`; unsupported names or missing
required parameters never fall back to nonrobust computation.

GLS takes a relative error covariance structure `sigma`: `Var(error) = scale * sigma`.
It estimates scale from whitened residual sums of squares divided by residual
degrees of freedom. Parameter covariance includes that scale; coefficient tests
use Student-t and the overall test uses F. The report labels this estimated-scale
contract. `fit_regression(Gls)` supplies identity structure and therefore agrees
with ordinary OLS inference. A fully known absolute covariance mode is not exposed.

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
