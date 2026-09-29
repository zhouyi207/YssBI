# yss-sci

> Status: Current
> Scope: 按统计领域和方法族组织的数值算法，以及 SCI 模块分类约定
> Canonical owners: 本 crate 源码拥有算法；节点目录拥有导航分类，SCI Contract 拥有中立契约
> Update when: 数值算法归属、领域模块或计算契约改变时

Statistical models and numerical algorithms over `yss-sci-linalg` matrices, vectors
and numeric slices. Matrix arithmetic, checked factorizations and rank conventions
use `yss-sci-linalg`; this crate does not depend on faer. Public computation contracts
use `yss-sci-contract`. The crate has no Arrow, Polars or chrono dependency.
Tabular input alignment and transformations belong to `yss-sci-runtime::preprocessing`.

## Domain organization

SCI, Runtime and Contract use the same domain names for capabilities they own.
The domains follow the [node catalog](../yss-node-catalog/README.md)'s main
categories; a category gets a module when it has an implementation or contract.
SCI does not depend on the catalog or use node IDs to select algorithms.

| Node category / capability               | SCI module                                   | Responsibility                                                                |
| ---------------------------------------- | -------------------------------------------- | ----------------------------------------------------------------------------- |
| Regression                               | `regression::linear`, `regression::discrete` | OLS/WLS/GLS/Prais and Logit/Probit, including their numerical fit projections |
| Panel models                             | `panel`                                      | Fixed/random effects, between and difference estimators, panel fit projection |
| Econometric and causal analysis          | `causal::iv`, `causal::did`                  | 2SLS/LIML, TWFE DID and DID randomization inference                           |
| Time series                              | `time_series`                                | ACF/PACF, ADF, VAR, VEC and cointegration rank                                |
| Hypothesis tests                         | `hypothesis`                                 | Constraint parsing, linearization and t/Wald tests                            |
| Model diagnostics                        | `diagnostics`                                | Heteroskedasticity, normality, RESET, VIF, leverage and serial correlation    |
| Probability distributions                | `distribution`                               | Sampling                                                                      |
| Descriptive statistics                   | `descriptive`                                | Individual and population-weighted Theil T                                    |
| Density estimation used by visualization | `density`                                    | Kernel-density numerical computation                                          |
| Data preprocessing                       | `preprocessing`                              | Numerical standardization; Arrow preparation stays in Runtime                 |

Each method owns its fitting, inference and postestimation modules. Fit results
retain model facts and coefficient inference; optional diagnostics and postestimation
are separate calls over those facts. A node's Fit/Summary/Predict stage does not
create a second algorithm owner. Runtime owns selected report projections; Contract
owns neutral options and results. Linalg remains a shared numerical foundation.

`regression::fit` only dispatches regression methods. Numerical entry points live
in `regression::linear::fit`, `regression::discrete::fit`, `panel::fit` and
`causal::iv::fit`. `time_series::models` prepares ADF/VAR/VEC computations.
`regression::design`, covariance and collinearity calculations are reused by
the estimators that need them. DID calls the existing panel estimator, which
continues to reuse OLS. `causal::did::fit_did` takes an explicit treatment vector;
`panel::fit::fit_panel` owns ordinary panel fitting.

These neutral fit entries consume shared binary/Prais and panel options, and IV
accepts multiple endogenous and excluded-instrument columns. Panel dispatch covers
FE/LSDV, entity first differences, entity/time/two-way RE FGLS and MLE, and
entity/time Between, rejecting unsupported covariance/effect combinations.
IV 2SLS and LIML share `IvEstimate` and coefficient statistics. Their first-stage,
overidentification and endogeneity analyses are separate calls in `causal::iv::fit`;
first-stage analysis reuses the same implementation for both estimators. Panel
estimators return `PanelFit` directly, grouping shared model/coefficient facts and
estimator-specific statistics without a parallel native result type.
DID randomization takes observed columns,
fits the TWFE treatment interaction, then permutes treatment at entity level with
execution checks between iterations.

Diagnostics use ordinary Rust submodules with explicit imports for shared
helpers. Residual normality belongs to `diagnostics::normality`; Durbin-Watson
and other serial correlation tests belong to `diagnostics::serial_correlation`.
`diagnostics::residual::diagnose` consumes the fitted linear result, including
original WLS weights, and dispatches BP/White/IM/RESET/VIF/leverage. GLS is rejected
for residual diagnostics requiring an untransformed or diagonal-weight design;
VIF remains a property of the original predictor design. Weighted RESET allocates
the full augmented design before inserting fitted-value or predictor powers.
The fitted-value BP/Koenker variants regress functions of residual squares on an
intercept and fitted values; residuals and fitted values have distinct argument
roles in both the OLS and WLS dispatch paths.

Panel first differences take entity IDs and original time values; they do not
require a second time-ID vector. First-stage IV summaries derive dimensions from
their matrices and receive covariance/estimator choices through `FirstStageOptions`.
Between estimators use conventional covariance; the panel dispatcher validates that
choice before calling the estimators, which take no covariance selector or parameters.

VAR and VEC return `VarFit` and `VecFit`, sharing equation/coefficient statistics.
Their `postestimation` modules compute residual serial tests and stability roots
from a fit; VAR additionally exposes lag exclusion, Granger, impulse responses and
variance decomposition. Response horizons and diagnostic lags belong to these calls,
not fit configuration. Fit results retain the design and residuals needed by those
analyses, without embedding a complete report or precomputed postestimation arrays.

`hypothesis::linear_hypothesis` owns constraint parsing, linearization, parameter order,
matrix construction, test selection and `at()` interpretation. It uses
`yss-math-expr` for generic syntax and validated t/Wald inputs in `hypothesis::linear_test`.
Project/result identity checks and report retrieval remain in Application.

## Theil T

`descriptive::theil_t` computes natural-log Theil T from individual values or
population-weighted group means. It validates nonnegative finite values/weights,
requires positive total weight and weighted income, and treats zero income as a
zero contribution. Separate log-space normalization and compensated sums avoid
overflow in population/income totals. Input scans and accumulation check execution
control every 1024 rows. Group means capture between-group inequality only.

## ACF/PACF

`time_series::acf_pacf::compute_acf_pacf` validates finite numerical input, scales before
centering and squaring, and uses compensated sums for the mean, variance and lag products.
It computes ACF once for the joint result, then uses the same correlations for
Durbin-Levinson PACF. Intermediate values and correlations are checked; nonpositive
recursion denominators and numerical breakdown return `ComputationFailed` rather than
fabricated coefficients. Independent `acf` and `pacf` return checked results and reuse
these numerical helpers. Controlled computation
checks cancellation/deadlines at stage boundaries and every 1024 loop elements;
callers retain worker scheduling and product lag budgets. The numerical lag bound
is `n - 1`, independent of the runtime report policy. Constant series preserve the
existing lag-zero-only ACF and empty PACF result.

## OLS model boundary

`regression::linear::ols` owns the model, its fitted result and its errors:

- `mod.rs`: `OLS`, `OlsFit` and `OlsFitError`.
- `fit.rs`: least-squares numerical work and the intermediate solution.
- `inference.rs`: covariance, degrees of freedom, tests and confidence intervals.

`OLS.config` uses the shared `yss_sci_contract::regression::OlsOptions`. Node
defaults and the runtime use the same definition. Named covariance inputs from
other estimators are parsed into that configuration; invalid combinations fail
explicitly, and unimplemented covariance modes retain their diagnostic failure.

`OlsFit` is the numerical authority for coefficients, fitted values, residuals,
rank and inference statistics. It does not carry a second copy of coefficients
inside a nested model. WLS, GLS, Prais, Logit, Probit and IV follow the same rule.
Runtime reports project these computed values.

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

Numerical estimator inputs use `yss_sci_linalg::Mat` and `Col`; cross-crate fit
contracts use ordinary vectors with documented row/column meanings. Call sites
use the shared OLS configuration. Result organization does not change solver or
convergence policy.

Classical hypothesis tests are organized under `hypothesis`: `sample_mean` owns mean, proportion, Poisson and equivalence tests; `categorical` owns count-table tests; `nonparametric` owns rank and sequence tests; and `variance` owns variance-homogeneity tests. These functions accept neutral contract requests and return common result records. The node catalog and kernel own graph-facing interfaces and dispatch.
