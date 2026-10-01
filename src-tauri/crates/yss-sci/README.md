# yss-sci

> Status: Current
> Scope: 按统计领域和方法族组织的数值算法，以及 SCI 模块分类约定
> Canonical owners: 本 crate 源码拥有算法；节点目录拥有导航分类，SCI Contract 拥有中立契约
> Update when: 数值算法归属、领域模块或计算契约改变时

Statistical models and numerical algorithms over `yss-sci-linalg` matrices, vectors
and numeric slices. Matrix arithmetic, checked factorizations and rank conventions
use `yss-sci-linalg`; this crate does not depend on faer. Public computation contracts
use `yss-sci-contract`. The crate has no Arrow, Polars or chrono dependency.
Tabular input alignment and transformations use `yss-database-engine` relation plans
through [Node Kernel](../yss-node-kernel/README.md).

## Domain organization

SCI, Runtime and Contract use the same domain names for capabilities they own.
The domains follow the [node catalog](../yss-node-catalog/README.md)'s main
categories; a category gets a module when it has an implementation or contract.
SCI does not depend on the catalog or use node IDs to select algorithms.

| Node category / capability               | SCI module                                                         | Responsibility                                                                                                     |
| ---------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------ |
| Regression                               | `regression::linear`, `regression::discrete`, `regression::models` | Linear/binary fits, robust/penalized/PLS, GLM and likelihood models, nonlinear designs and model-building analyses |
| Panel models                             | `panel`                                                            | Fixed/random effects, between and difference estimators, panel fit projection                                      |
| Econometric and causal analysis          | `causal::iv`, `causal::did`                                        | 2SLS/LIML, TWFE DID and DID randomization inference                                                                |
| Time series                              | `time_series`                                                      | ACF/PACF, ADF, VAR, VEC and cointegration rank                                                                     |
| Hypothesis tests                         | `hypothesis`                                                       | Constraint parsing, linearization and t/Wald tests                                                                 |
| Analysis of variance                     | `anova`                                                            | Factorial ANOVA/ANCOVA, MANOVA and complete within-subject designs                                                 |
| Correlation and agreement                | `association`                                                      | Paired/rank/partial correlation, Kappa, ICC, concordance, Ridit and rwg                                            |
| Multivariate analysis                    | `multivariate`                                                     | PCA, principal-axis factors, CCA, correspondence, LDA/QDA, RDA and classical MDS                                   |
| Model diagnostics                        | `diagnostics`                                                      | Heteroskedasticity, normality, RESET, VIF, leverage and serial correlation                                         |
| Probability distributions                | `distribution`                                                     | Sampling                                                                                                           |
| Descriptive statistics                   | `descriptive`                                                      | Empirical Gini, Dagum decomposition and Theil T                                                                    |
| Density estimation used by visualization | `density`                                                          | Kernel-density numerical computation                                                                               |
| Visualization plot data                  | `visualization`                                                    | Controlled distributions, paired points, category frequencies and matrix projections                               |

Each method owns its fitting, inference and postestimation modules. Fit results
retain model facts and coefficient inference; optional diagnostics and postestimation
are separate calls over those facts. A node's Fit/Summary/Predict stage does not
create a second algorithm owner. Runtime owns selected report projections; Contract
owns neutral options and results. Linalg remains a shared numerical foundation.

Numerical entry points live in `regression::linear::fit`,
`regression::discrete::fit`, `panel::fit` and
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

## Additional regression estimators

`regression::models` owns controlled estimators over neutral numeric columns:
Huber/Tukey M-estimation with MAD/H1 inference, Ridge/Lasso with an unpenalized
intercept, PLS1, quantile IRLS with IID density covariance, curve families, bounded
custom-formula nonlinear least squares, Deming with linear-time delete-one
jackknife, and restricted cubic spline bases with an OLS analysis.

GLM supports Gaussian identity/log, binomial logit/probit/cloglog and log-linked
Poisson/Gamma/inverse Gaussian. Fractional response reuses the mean equation with
HC0 score-sandwich inference. Likelihood fitting covers NB2, ZIP/ZINB, normal
left/two-bound Tobit, constant-precision Beta, multinomial/ordered logit and
stratified binary conditional logit. Firth solves the adjusted logistic score.
Conditional normalizers use log-domain case-count dynamic programming and omit
homogeneous strata; they do not identify absolute probabilities.

Shared numerical preparation scales designs and restores coefficient/covariance
units. Likelihood models use a controlled line-search optimizer and complete
observed Hessians, including mixture cross-blocks. Nonlinear fitting uses the
existing expression parser, damped Jacobian steps and active parameter bounds;
boundary inference is unavailable. Computation failure or nonconvergence remains
an error, not a successful placeholder fit. No coefficient p-values are invented
for Ridge/Lasso/PLS. Iteration checks also cover observation scans and objective
evaluations; decompositions and sorting are checked at their boundaries.

Block-entry, stepwise AIC/BIC, single-variable/all-variable and grouped analyses
reuse existing OLS. Single-threshold regression searches a bounded candidate grid
and treats the selected threshold as fixed for conditional coefficient inference.
The neutral structured results retain actual models, original predictor/row
positions, selection history, category probabilities and method-specific facts.
Fixtures provide reproducible SciPy/statsmodels coefficients and inference;
separate kernel/graph checks cover labels, budgets, alignment and authoring defaults.

## Correlation and agreement

`association::correlation` scales and centers finite observations before computing
correlations. Partial correlation residualizes against full-rank centered controls
through checked Linalg factors; degenerate controls/residuals fail. Rank methods use
average ranks and Kendall tau-b uses a Fenwick tree, including both marginal tie
corrections. Exact inference enumerates position permutations for at most nine
observations (including ties); larger samples use documented approximations.

`association::agreement` owns weighted Cohen and Fleiss Kappa, six ICC definitions,
Bland–Altman limits/intervals, tie-corrected W, and rwg with an explicit null variance.
Kappa intervals use delta variance; ICC intervals use F distributions; Bland–Altman
statistics use complete observations while display points are bounded. Negative
estimates are retained where defined; nonidentifiable coefficients fail and
unavailable inference uses `None`. `association::ridit` compares independent ordered
samples and uses tie-corrected rank inference. Input scans and numerical loops check
execution control; sorts and Linalg decompositions are checked at their boundaries.
Reference fixtures identify their SciPy/statsmodels versions and cover coefficient,
test and interval values separately from kernel metadata and graph alignment tests.

## Multivariate analysis

`multivariate` owns centered/scaled preparation, symmetric covariance spectra,
principal-axis factor extraction and orthogonal varimax, whitened CCA, simple
correspondence analysis, Gaussian LDA/QDA, multivariate least-squares RDA and
classical metric MDS. It uses only checked Linalg matrices/factors and shared
`yss-sci-contract::multivariate` options/results. Original means, scales, eigenvalues
and inertia remain explicit; coordinate matrices are row-major independent outputs.

PCA can use sample-SD standardization or original-unit covariance. Factor models
require identification, positive-definite correlation and convergence; regression
scores and KMO/Bartlett are distinct from a model-fit test. CCA keeps all roots for
sequential Wilks/Bartlett inference even when fewer axes are retained. Exact perfect
roots retain zero lambda with unavailable finite inference. CA accepts nonnegative
weights and retains null inertia proportions for independent tables.

LDA/QDA standardize training features, apply optional isotropic covariance shrinkage,
and classify aligned optional new observations using training means/scales and
empirical/equal priors. Training confusion is not cross-validation. RDA returns
fitted-response scores, inertia/R-squared and optional reproducible whole-row
permutation inference. MDS accepts observation Euclidean distances or validated
square dissimilarities, reports negative inertia and uses positive embedding axes.

Controlled loops check cancellation/deadlines, with checks around decompositions.
Kernel owns complete workspace admission and tabular labels, not SCI. No graph,
Arrow or backend-specific type crosses the neutral scientific result contract.

## Analysis of variance

`anova` owns sum-contrast categorical designs with additive centered ANCOVA
covariates. Main-effect/full-factorial models support type I sequential, type II
marginality and type III adjusted nested-model tests, sharing checked rank and
Cholesky least squares from Linalg. Factor terms use dynamic index sets, including
additive designs with more than 64 factors, and generate interactions on demand.
Full designs must be identifiable with positive
residual degrees of freedom; missing/nonfinite observations are rejected.

MANOVA reuses those designs and residual SSCP matrices, whitens each hypothesis
with residual Cholesky and calculates symmetric eigenvalues for Wilks, Pillai,
Hotelling–Lawley and Roy statistics and their F approximations. Invalid small-sample
approximation degrees use optional inference fields; singular response covariance
is an input failure. SSCP arrays retain original response units and input order.

Repeated measures require complete balanced within-subject factorial cells,
using tensor products of orthonormal Helmert contrasts to separate each effect
from its subject-interaction error. Greenhouse–Geisser epsilon is computed from
each effect's contrast covariance and adjusts reference degrees without changing F.
Input scans, term construction and computations check cancellation/deadlines;
matrix decompositions are not cooperatively interrupted.

## Inequality measures

`descriptive::gini` uses equal observation weights without a small-sample correction.
Positive adjacent gaps in sorted, maximum-scaled values replace cancellation-prone
weighted rank differences; compensated sums keep finite inputs usable even when
their unscaled total would overflow. Negative/missing/nonfinite values and a zero
overall mean are rejected.

`dagum_gini` returns within, net between and transvariation contributions, their
shares, subgroup statistics and pairwise rows through `yss-sci-contract::descriptive`.
Directed differences are integrated over merged sorted group samples without an
observation-pair matrix. Complexity is O(n log n + kn) for n observations and k groups;
group-pair output is admitted against the caller's workspace budget. Undefined zero-subgroup statistics and zero-Gini
contribution shares use `None`, not NaN. Sorting boundaries, input scans and numerical
loops check cancellation/deadlines. Sort itself is not cooperatively interruptible.

`descriptive::theil_t` computes natural-log Theil T from individual values or
population-weighted group means. It validates nonnegative finite values/weights,
requires positive total weight and weighted income, and treats zero income as a
zero contribution. Separate log-space normalization and compensated sums avoid
overflow in population/income totals. Input scans and accumulation check execution
control every 1024 rows. Group means capture between-group inequality only.

## Visualization

`visualization/` owns numerical plot data in `distribution`, `points`,
`categorical` and `matrix`. Neutral records live in `yss-sci-contract::visualization`;
Runtime forwards functions and execution control. KDE, Pearson correlation and
ACF/PACF reuse their existing numerical owners. Node Kernel prepares tabular inputs;
D3 owns pixels, axes, colors and word placement. SCI retains no graph or window state.

Coefficient and Pareto results retain every term/category; the presentation layer
paginates those displays. Distribution summaries and ROC AUC use complete samples. Point displays are bounded
at 2048, heatmaps at 128 rows and grouped displays at 64 columns; sampling metadata
preserves original observation counts. Coefficient intervals consume fitted model
facts and residual degrees of freedom without refitting.

The `visualization` example emits actual SCI payloads for the frontend contract
fixture and manual D3 previews. Box whiskers follow the [NIST convention](https://www.itl.nist.gov/div898/handbook/eda/section3/boxplot.htm);
normal probability plots document their Hazen plotting positions in node help.

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
contract. An explicitly supplied identity `sigma` agrees with ordinary OLS
inference. A fully known absolute covariance mode is not exposed.

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

Binary postestimation lives in `regression::discrete::postestimation`: Logit odds
ratios transform coefficient intervals and retain the coefficient-null z test;
Logit/Probit continuous-regressor margins support AME, MEM, explicit at values and
four derivative/elasticity scales with analytic delta-method covariance. Margins
check shared execution control in observation loops; Kernel admits retained design,
quadratic workspace and cubic work before dispatch. Classifier diagnostics use
`p >= cutoff` and optional values for zero-denominator rates. Probit IRLS uses the
inverse-link derivative in its working response; default MLE covariance uses
observed information. A checked-in statsmodels 0.14.6 fixture verifies both links.

Panel fits retain the actual estimation-scale design/response/fitted/residual
sample with its transformation name and source-row groups. Estimation-scale
prediction reuses those coefficients; it does not claim to predict absorbed effects
for new entities. Selected-lag VAR fits retain exogenous labels, covariance divisor
choice and complete source rows. Prais retains the full rho iteration sequence.
