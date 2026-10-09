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

## Survival analysis

`survival` owns Kaplan–Meier/Nelson–Aalen curves, Log-rank, Aalen–Johansen
incidence, Efron/Breslow Cox (including counting-process and stratified subgroup
designs), four right-censored AFT distributions, calibration, decision curves and
Cox nomogram scales. It reuses regression design/optimization/inference helpers
and Linalg decompositions. Times are positive; no rows are silently dropped.
Observation arrays preserve shape, nonfinite and data-domain violations, including
binary events/treatments, counting-process intervals, group identifiability and
predicted probabilities. Supplied Cox model layout and numeric fields retain the
same input roles. Invalid horizons, bin/tick counts, thresholds and iteration
settings remain parameter violations; numerical breakdown remains computation failure.
Event/treatment, interval, group and prediction scans check execution control in chunks.
Curve, Log-rank, Cox stratum/subject and subgroup preparation share an exact,
ordered group-to-row index built in one pass. They consume each group's rows
instead of rescanning the full input per group; curve sorting reuses its owned
row buffer. The index requires linear row storage within the admitted workspace.
Cox baselines are centered Breslow estimates; AFT covariance includes log(scale).
`tests/survival_category.rs` checks independently generated statsmodels/SciPy
references and risk-set, interval, prediction and cancellation conventions.
Regenerate production plot payloads for parser checks and manual previews with
`cargo run -p yss-sci --example survival -- react/src/tests/fixtures/node-system-contracts/survival-payloads.json`.

## Domain organization

`decision` separates criterion preparation, objective weighting and alternative
ranking. Entropy, CRITIC, variation-coefficient and inverse-multiple-correlation
weights share normalization/admission conventions; the last reuses SVD collinearity.
Composite scores, TOPSIS, ideal-reference grey relations, WRSR and efficacy scores
share weight normalization and the existing controlled average-rank computation.
Entropy TOPSIS composes those implementations. Weight vectors follow criterion
order; observation scores retain every input row. References use NumPy, SciPy,
statsmodels and PyMCDM; no external Python dependency is used at runtime.

`decision/compromise` owns VIKOR's S/R/Q scores and both compromise-set conditions.
`decision/systems` owns normalized subsystem coupling and criterion obstacle shares.
Undefined all-zero coupling and zero-denominator obstacle percentages remain optional
numbers rather than NaN or a fabricated share. Coupling degree uses its continuous
zero limit when all subsystem indices are zero.

`decision/preferences` owns NPS category proportions, classic KANO paired-answer
classification and coefficients, and RFM scores using the shared average-rank
implementation. RFM uses midpoint-rank quintiles, preserves ties, reverses recency,
and reports row-level scores independently of marketing labels.

`decision/pricing` constructs right-ECDF price curves, interpolated intersections
and coincident-price intervals with explicit original/narrower range definitions.
`decision/reach` exhaustively evaluates fixed-size TURF subsets using packed
observation sets, preserving cancellation and reporting optimum ties.

`decision/hierarchy` owns principal-eigenvector AHP and complementary-matrix FAHP.
Missing reference RI leaves CR optional without bounding the matrix order.
`decision/influence` owns convergent DEMATEL total influence and ISM transitive
closure/SCC level extraction. Linalg remains the sole matrix-factorization owner.
`decision/fuzzy` composes normalized criterion/grade memberships with four operators.
`decision/experts` summarizes one Delphi round and reuses tie-corrected Kendall W.
Type-7 sample quantiles now belong to `descriptive/quantiles` and are shared by
Delphi, distribution plots, item discrimination, path bootstrap intervals and
the Mood/Brown-Forsythe medians. Halfway interpolation uses the standard-library
midpoint so finite large endpoints do not overflow and equal subnormal endpoints
do not round separately to zero. Undefined coefficients remain optional values.
`decision/conjoint` fits additive ratings with the shared least-squares solver,
then transforms coefficients and covariance to within-attribute zero-sum utilities.
Saturated identifiable designs return utilities without residual-based standard errors.

`psychometrics` separates scale reliability, total-score tail discrimination and
expert relevance. Alpha and corrected item-total statistics share linear-memory
score moments; item analysis reuses type-7 quantiles and Welch inference, keeping
boundary ties together. `multivariate/adequacy` owns KMO, per-item MSA and Bartlett
diagnostics shared by validity screening and exploratory factor extraction.
Neither screening nor alpha is reported as proof of construct validity. Undefined
correlations, deleted-item alpha and tail tests remain optional values.
Item discrimination propagates Welch's shared computation errors directly.

`quality/process` shares scaled measurement moments, moving-range variation and
pooled subgroup sigma. `quality/control` builds complete I/MR plots and signal
rows; `capability` distinguishes within Cp/Cpk from overall Pp/Ppk and target Cpm.
`quality/gage` owns balanced crossed random-effects ANOVA and variance components;
its part/operator denominators are interaction mean squares, unlike fixed-effects
ANOVA. Interaction pooling is explicit, negative estimates are listed when
truncated, and undefined statistics remain optional. These entries do not infer
process stability or engineering acceptance from a single computed index.

`doe/design` generates equal-level full factorial and orthogonal arrays; composite levels
use full factorial construction. `doe/uniform` searches centered Latin hypercubes using
squared centered L2 discrepancy. `design_dimensions` validates the same specification
before the adapter admits matrix memory; generation checks cancellation/deadlines.

`doe/surface` expands range-coded full quadratic terms, reuses OLS and classifies
stationary geometry through the Linalg symmetric eigensolver. `doe/dose` reuses
the nonlinear formula solver for four-parameter log-logistic least squares, including
zero-dose limits, log-positive Hill/ED50 parameters and local inference.

`doe/range` owns compensated factor-level aggregation, raw effect ranges and
exact contingency-count checks for pairwise orthogonality. Descriptive summaries
remain available for unbalanced designs, with explicit replication and level-count
facts; they do not pretend to be ANOVA inference or a validated joint optimum.

`inference` owns normal/t confidence intervals, pooled-ANOVA/Welch pairwise
contrasts with Holm/Bonferroni adjustment, and OLS CR1 cluster inference. Cluster
covariance reuses the linear estimator; coefficient tests use cluster-count df.
Cluster admission reports incompatible group lengths as `ShapeMismatch`, insufficient
samples as `EmptyInput`, and fewer than two clusters as `DataOutOfRange`. Group scans
check execution control, and downstream OLS errors retain their shared violation
through Contract's neutral conversion.
`regression/postestimation` owns shared evaluation grids, binary-link derivatives
and Delta variance for adjusted means and binary marginal effects. Evaluation
retains fitted row order and applies explicit column overrides without rebuilding
transformed designs. Conventional linear means use residual-df t inference;
robust linear and binary means use normal inference. Independent references are
in `tests/fixtures/inference_reference.*` and `postestimation_reference.*`.

SCI, Runtime and Contract use the same domain names for capabilities they own.
The domains follow the [node catalog](../yss-node-catalog/README.md)'s main
categories; a category gets a module when it has an implementation or contract.
SCI does not depend on the catalog or use node IDs to select algorithms.

| Node category / capability               | SCI module                                                         | Responsibility                                                                                                           |
| ---------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| Regression                               | `regression::linear`, `regression::discrete`, `regression::models` | Linear/binary fits, robust/penalized/PLS, GLM and likelihood models, nonlinear designs and model-building analyses       |
| Panel models                             | `panel`                                                            | Fixed/random effects, between and difference estimators, panel fit projection                                            |
| Spatial analysis                         | `spatial`                                                          | Coordinate weights, global Moran, OLS/SLX and Gaussian SLM/SEM/SAC/SDM/SDEM, balanced entity fixed-effect spatial panels |
| Econometric and causal analysis          | `causal`                                                           | IV/DID, linear GMM, selection/frontier/SUR, local RDD, treatment effects, group interactions and synthetic controls      |
| Time series                              | `time_series`                                                      | ACF/PACF, ADF, VAR, VEC and cointegration rank                                                                           |
| Hypothesis tests                         | `hypothesis`                                                       | Constraint parsing, linearization and t/Wald tests                                                                       |
| Analysis of variance                     | `anova`                                                            | Factorial ANOVA/ANCOVA, MANOVA and complete within-subject designs                                                       |
| Correlation and agreement                | `association`                                                      | Paired/rank/partial correlation, Kappa, ICC, concordance, Ridit and rwg                                                  |
| Multivariate analysis                    | `multivariate`                                                     | PCA, principal-axis factors, CCA, correspondence, LDA/QDA, RDA and classical MDS                                         |
| Longitudinal and multilevel models       | `longitudinal`                                                     | GEE; Gaussian ML/REML with independent random slopes and nested/crossed intercepts; random-intercept GLMM Laplace ML     |
| Model diagnostics                        | `diagnostics`                                                      | Heteroskedasticity, normality, RESET, VIF, leverage and serial correlation                                               |
| Probability distributions                | `distribution`                                                     | Sampling and shared F upper tails                                                                                        |
| Descriptive statistics                   | `descriptive`                                                      | Empirical Gini, Dagum decomposition and Theil T                                                                          |
| Density estimation used by visualization | `density`                                                          | Kernel-density numerical computation                                                                                     |
| Visualization plot data                  | `visualization`                                                    | Controlled distributions, paired points, category frequencies and matrix projections                                     |

`meta` separates effect conversion, inverse-variance regression, asymmetry and
sensitivity diagnostics, and plot data. Intercept-only pooling and moderator
models share one weighted-fit implementation; Paule–Mandel estimates residual
heterogeneity with moderators included. Existing regression designs, coefficient
inference, Kendall correlation and plot payloads are reused. Independent
NumPy/SciPy/statsmodels references live in `tests/fixtures/meta_reference.*`.

Each method owns its fitting, inference and postestimation modules. Fit results
retain model facts and coefficient inference; optional diagnostics and postestimation
are separate calls over those facts. A node's Fit/Summary/Predict stage does not
create a second algorithm owner. Runtime owns selected report projections; Contract
owns neutral options and results. Linalg remains a shared numerical foundation.

`spatial` reuses regression design scaling, coefficient inference, controlled
optimization and Linalg eigensystems/solves. Weights have zero diagonals and
nonnegative entries; KNN, distance bands and inverse distance support union
symmetrization and optional row normalization. Cross sections use Gaussian ML
with full observed information, except OLS/SLX which use residual-df covariance.
Spatial coefficients stay inside the sufficient stability interval set by the
maximum row sum. Balanced panels use orthonormal Helmert time contrasts with
`N*(T-1)` likelihood observations and restore entity effects afterward. Innovation
Moran indices and spatial impacts are descriptive point results; standalone Moran
provides normal/randomization moments and seeded two-sided permutations. Dense
workspace admission belongs to Kernel. Independent NumPy/SciPy references and
their generator live in `tests/fixtures/spatial_category_reference.*`.

Numerical entry points live in `regression::linear::fit`,
`regression::discrete::fit`, `panel::fit` and
`causal::iv::fit`. `time_series::models` prepares ADF/VAR/VEC computations.
ADF admits its retained observations against the actual regression-column count
before constructing the design, requiring positive residual degrees. Its trend
specification includes a constant. Undefined or nonfinite regression statistics
return a fit error before probability evaluation. Auxiliary coefficient inference
uses the computed standard errors without an absolute floor, preserving results
when the response unit changes. One Student-t reference serves both Drift and
the auxiliary table; the design is built directly without duplicate row buffers
or matrix copies. Panel Fisher tests reuse this same regression owner while
retaining their existing MacKinnon calibration and panel admission.
`regression::design`, covariance and collinearity calculations are reused by
the estimators that need them. DID calls the existing panel estimator, which
continues to reuse OLS. `causal::did::fit_did` takes an explicit treatment vector;
`panel::fit::fit_panel` owns ordinary panel fitting.
OLS, WLS, GLS and Prais return their existing fit error when coefficient division
produces a NaN t-statistic, before calling the Student-t distribution. Prais also
rejects a NaN F-statistic before its Fisher distribution call. Infinite statistics
keep the existing distribution path; adapters retain their finite-output validation.
`distribution::fisher_snedecor_sf` owns the shared F upper-tail calculation for
ANOVA, regression/Wald, panel and IV inference, ICC, measurement studies and
central-F power. It reuses the validated native distribution and reciprocal F
CDF with swapped degrees when the native survival function would subtract a
beta argument near one. The lower-statistic branch retains native SF, including
zero statistics, without overflowing a reciprocal. Domain callers retain their
parameter/error and execution-control boundaries; none duplicates the tail calculation.

`distribution::student_t_probability` owns directed probabilities for standard
Student-t references used by sample-mean/equivalence tests, coefficient constraints,
regression and panel/IV inference, correlation, pairwise comparisons, path effects
and central-t power. Ordinary tails use SF/CDF directly. When the incomplete-beta
argument is below floating-point epsilon, the shared owner evaluates its leading
integral term in log space, avoiding statistic-square overflow and premature
subnormal rounding. A fourth-moment bound handles probabilities below the
representable range without replacing positive tails with a fixed cutoff.
The shared calculation retains fractional Welch degrees. Model coefficient tables
also reuse it for Student references, including Knapp–Hartung Meta and
design-based survey inference.
[The beta integral](https://dlmf.nist.gov/8.17) and
[Student density](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.t.html)
define the calculation; native quantiles and noncentral algorithms retain their owners.

`distribution::normal_two_sided_p` owns standard-normal two-sided inference.
It evaluates `erfc(abs(z)/sqrt(2))` directly, avoiding subtraction from a rounded
CDF and the intermediate halving that can erase a representable subnormal.
Binary, IV, random/dynamic panel, model coefficient, asymptotic constraint,
correlation/rank and VAR/VEC inference reuse this calculation. The time-series
boundary retains its existing rejection of nonfinite statistics.
Chi-square upper tails reuse the validated native distribution's SF directly,
including diagnostics, binary likelihood ratios, IV, random panel and VAR/VEC
postestimation; these callers keep their existing degrees, unavailable-result
and control contracts. No extra Chi-square wrapper or reference model is introduced.
The [normal/error-function identity](https://dlmf.nist.gov/7.20.iii) and
[Chi-square density](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.chi2.html)
provide the scientific references.

These neutral fit entries consume shared binary/Prais and panel options, and IV
accepts multiple endogenous and excluded-instrument columns. Panel dispatch covers
FE/LSDV, entity first differences, entity/time/two-way RE FGLS and MLE, and
entity/time Between, rejecting unsupported covariance/effect combinations.
IV 2SLS and LIML share the numerical `causal::iv::IvModel` input and its existing
`PreparedIvDesign` producer. `fit_2sls` and `fit_liml` consume the same instrument
matrix, inverse, projected regressors and observed structural design. Their method
implementations and shared postestimation live together under `causal/iv`; the
neutral `causal::iv::fit` entry prepares one numerical input before dispatch.
Numerical input carries no display labels: Runtime restores source labels on the
shared fitted model and first-stage records. Direct matrix construction and borrowed
included-instrument columns replace duplicate row buffers, instrument decomposition
and matrix/vector copies in the estimators.
LIML keeps its k-class cross-product inverse for covariance but uses instrument-
projected score rows, structural residuals and projected leverage for HC2/HC3.
The same projected scores feed HC0/HC1, cluster and serially robust covariance.
The kappa eigenproblem is reversed and Cholesky-whitened on the positive-definite
included-regressor residual cross-product. Its largest reciprocal root retains
singular instrument residual covariances without treating a null direction as
kappa zero. Residual cross-products are formed directly from projected columns,
without subtracting nearly equal cross-products or using an absolute eigenvalue
cutoff. The endogenous projections reuse the prepared IV design. The projection
ordering enforces kappa >= 1 under roundoff; undefined roots fail before inference.
At exact identification its coefficient/covariance results agree with 2SLS;
overidentified LIML retains its estimated kappa and corresponding cross-product.
Both estimators share `IvEstimate` and coefficient statistics. The existing
estimate owner computes structural and first-stage R²/adjusted R² once through
one borrowed-input calculation. It centers only models with an intercept and uses
the actual residual degrees and total degrees `n-constant`. A common scale for
centered observations and structural residuals cancels in RSS/TSS, preserving
microscopic response units without fixed variance floors or underflowing mean
squares. Negative IV R² remains valid; zero/undefined total variation or nonfinite
metrics returns scientific failure before encoding. No observation buffers or
parallel statistics model are introduced. The same owner computes both estimators'
coefficient and joint inference from the selected covariance. Default coefficient tests and 95% intervals use a normal
reference; `small=true` uses Student-t with the structural residual degrees.
The joint test excludes an estimated intercept: its Wald statistic uses chi-square
by default, or F after division by the number of restrictions with `small=true`.
`statistics.modelTest` carries the actual distribution, statistic, degrees and
p-value. Invalid coefficient variances and undefined or nonfinite statistics and
intervals return a scientific failure before probability evaluation or serialization.
These conventions follow the [Stata IV manual](https://www.stata.com/manuals/rivregress.pdf).
Their first-stage, overidentification and endogeneity analyses are separate calls
in `causal::iv::fit`. LIML overidentification borrows the fitted kappa and validated
sample/design counts: Anderson–Rubin is `n*(kappa-1)` with chi-square reference, and
Basmann F is `(kappa-1)*(n-k_z)/m` with degrees `m, n-k_z`, where `m` is excluded
instruments minus endogenous regressors. It reuses the existing diagnostic-fit
validation without reconstructing matrices, coefficients or structural residuals.
Exact identification, robust covariance and nonpositive instrument-regression
residual degrees retain unavailable results; invalid fitted kappa or overflowing
statistics return scientific failure. The numerical-model overidentification method is removed; the neutral fitted-model entry owns this analysis.
First-stage analysis reuses the same implementation for both estimators.
It requires positive first-stage residual degrees before preparing the design;
saturated instrument regressions return a scientific failure while the structural
fit and summaries that omit this analysis remain available. The existing IV design
preparation owns instrument coefficients, fitted endogenous columns and the
instrument cross-product inverse; inference consumes those facts once.
Included-instrument residualization applies the projection directly to the required
columns, and instrument residuals reuse the fitted columns. This avoids observation-
square projection matrices, repeated first-stage fitting/factorization and per-row
mean scans. Cragg–Donald inference uses Cholesky whitening of its generalized
symmetric eigenproblem, preserving column-order and unit invariance when residual
covariances are correlated. The excluded-instrument normalization and Stock–Yogo
critical tables retain their existing conventions. See the
[LAPACK reduction](https://netlib.org/lapack/lug/node54.html) for the symmetric
generalized eigenproblem and the worked first-stage examples in the
[Stata postestimation manual](https://www.stata.com/manuals/rivregresspostestimation.pdf).
First-stage equations retain the selected OLS covariance in their shared
`RegressionCoefficientStatistics` inference record and expose `df_residual=n-k_z`.
Coefficient tests and 95% intervals use Student-t with those degrees; the excluded-
instrument Wald test reuses that covariance and reports F with numerator degrees
equal to the excluded-instrument count and denominator degrees `n-k_z`, independent
of the structural `small` option. No-constant equations use uncentered total variation
and its corresponding adjusted R². Zero or undefined variance returns a scientific
failure when first-stage analysis is requested; positive tiny variances retain their
units without a fixed floor. Summaries omitting first-stage analysis remain available.
Rank-zero traditional Hausman tests and endogeneity bundles without positive
Wu denominator degrees of freedom likewise remain unavailable in the existing
typed options, rather than returning NaN for later JSON conversion.
First-stage residualization preserves observation rows when excluding each endogenous
regressor, including models with three or more endogenous columns; reordering those
columns reorders their Shea partial-R² results without changing their identities.
Panel estimators return `PanelFit` directly, grouping shared model/coefficient facts and
estimator-specific statistics without a parallel native result type.
Two-way random-effects MLE keeps coefficients in retained-column order. Its likelihood
and iterative residual calculations use that same mapping after collinear columns
are removed, including the final and pooled likelihoods.
DID randomization takes observed columns,
fits the TWFE treatment interaction, then permutes treatment at entity level with
execution checks between iterations.

Diagnostics use ordinary Rust submodules with explicit imports for shared
helpers. Residual normality belongs to `diagnostics::normality`; Durbin-Watson
and other serial correlation tests belong to `diagnostics::serial_correlation`.
Breusch-Godfrey requires one equally sized, nonempty design row per residual before
building either auxiliary matrix. Invalid shapes retain its existing `None` result;
Runtime can still return the independent Durbin-Watson and Ljung-Box results.
`diagnostics::{comparison,influence,design,reclassification}` owns Gaussian/binary
likelihood criteria and nested tests, OLS/WLS influence, collinearity/Harman PCA
and paired binary NRI/IDI. Gaussian criteria count the estimated error variance;
comparisons check reconstructed response rows, proportional precision weights and
design-span nesting. Collinearity uses a thin SVD of the scaled design to retain
small singular values without allocating a square observation matrix. Undefined
influence and unbounded collinearity use optional values.
`survival::cox::proportional_hazards` reuses the fitted Efron/Breslow event
contributions for efficient time-interaction scores, including nuisance adjustment.
Independent NumPy/statsmodels references and their generator are in
`tests/fixtures/diagnostics_reference.*`; runtime golden tests exercise fitted-model inputs.
`diagnostics::residual::diagnose` consumes the fitted linear result, including
original WLS weights, and dispatches BP/White/IM/RESET/VIF/leverage. GLS is rejected
for residual diagnostics requiring an untransformed or diagonal-weight design;
VIF remains a property of the original predictor design. Weighted RESET allocates
the full augmented design before inserting fitted-value or predictor powers.
White and IM require an intercept in the fitted model. Dispatch checks the model's
existing `constant` flag rather than treating its first predictor as an intercept
or adding a new one. Their auxiliary rank and projection calculations reuse one
SVD of each design. Weighted IM weights only the heteroskedasticity component;
its skewness and kurtosis components retain the existing unweighted convention.
The fitted-value BP/Koenker variants regress functions of residual squares on an
intercept and fitted values; residuals and fitted values have distinct argument
roles in both the OLS and WLS dispatch paths.

Panel first differences take entity IDs and original time values; they do not
require a second time-ID vector. First-stage IV summaries derive dimensions from
their matrices and receive covariance/estimator choices through `FirstStageOptions`.
FE and RE share the one-way group-centering calculation in `panel::data` for entity
and time effects. It retains observation order and the existing NaN handling;
each estimator keeps its own input admission and fitting workflow.
Between estimators use conventional covariance; the panel dispatcher validates that
choice before calling the estimators, which take no covariance selector or parameters.

VAR and VEC return `VarFit` and `VecFit`, sharing equation/coefficient statistics.
Their `postestimation` modules compute residual serial tests and stability roots
from a fit; VAR additionally exposes lag exclusion, Granger, impulse responses and
variance decomposition. Response horizons and diagnostic lags belong to these calls,
not fit configuration. Fit results retain the design and residuals needed by those
analyses, without embedding a complete report or precomputed postestimation arrays.
VAR impulse responses and variance decomposition construct lag matrices only through
the requested horizon; stability still uses the full selected-lag companion matrix.
Lag-order selection reuses its owned observations across fits on the common sample,
and VAR fitting/postestimation borrow existing local matrices instead of copying them
solely for the next matrix operation.
VEC fitting and rank tests share the Johansen sample boundary: the observation count
must exceed the positive lag order before differencing or subtracting that order.
They return their existing error on an empty or exhausted sample. VEC stages and
postestimation likewise borrow local matrices for read-only linear algebra; residual
and in-place factorization copies remain where the original matrix is still needed.

`hypothesis::linear_hypothesis` owns constraint parsing, linearization, parameter order,
matrix construction, test selection and `at()` interpretation. It uses
`yss-math-expr` for generic syntax and validated t/Wald inputs in `hypothesis::linear_test`.
The matrix-level t/Wald routines are private to that boundary. Nonpositive or NaN
contrast variance fails before taking its square root, preserving the existing
typed computation-failure result instead of passing NaN to the reference distribution.
Project/result identity checks and report retrieval remain in Application.

## Univariate time series

`time_series::forecast` owns conditional ARIMA/SARIMA least squares with stable
AR/invertible MA factors and integrated Gaussian prediction intervals, fixed-initial
SES/additive ETS/Holt–Winters smoothing, Gaussian ARCH/GARCH/EGARCH/GJR likelihood,
two-step ECM, GM(1,1), discrete Markov forecasts and PP/KPSS tests. It reuses the
regression optimizer and least-squares design/inference; shared MacKinnon tau
calibration lives in `time_series::mackinnon` and is also used by panel tests.
ADF without a constant or with a trend uses that same p-value calibration;
the existing drift Student-t convention, critical values and auxiliary regression
remain explicit in its result.
The shared ADF regression rejects lags outside the sample before adding the lag
offset or allocating differences, including for direct SCI and Runtime callers.
ECM suppresses ordinary long-run OLS inference for cointegrating equations.
EGARCH averages simulated variances with an explicit seed; other volatility models
use analytic conditional-variance forecasts. KPSS reports its table-tail bounds.
All new entry points accept execution control, reject invalid domains and preserve
source-row alignment; admissibility comes from model identification and caller-owned
workspace budgets rather than a fixed observation ceiling. Numerical references in
`tests/fixtures/time_series_reference.py` use SciPy, statsmodels and arch; the Rust
integration target is `time_series_category`.

## Additional econometric and causal estimators

`causal::econometrics` owns one/two-step linear IV GMM with HC0 sandwich and
two-step overidentification inference, excluded-variable Heckman two-step,
normal–half-normal production/cost likelihood and two-stage SUR with distinct
equation designs. `causal::designs` owns sharp local-linear RDD with fixed-bandwidth
HC3 inference, HC3 treatment-by-group Wald tests and pre-period-only simplex
synthetic controls. SUR solves block normal equations without an observation-square
Kronecker covariance; synthetic controls retain all periods only for predictions.

`causal::treatment` reuses controlled Logit and OLS for nearest-neighbour propensity
matching with replacement, normalized Hájek IPW, RA and AIPW. Matching includes exact
distance ties and rejects caliper failures instead of changing the estimand by
dropping rows. It returns no naive matching inference. Other treatment estimators
and Heckman optionally resample independent rows and refit all nuisance stages;
failed bootstrap fits abort rather than biasing inference by being discarded.
The default is point estimation; bootstrap inference uses normal intervals.

Shared regression designs, coefficient tables, stable normal log-CDF and controlled
optimization remain owned by `regression::models::common`; all matrix arithmetic
stays behind Linalg. Algorithms check cancellation/deadlines between scans and
iterations. Nonidentification, nonconvergence and nonfinite computations fail;
frontier boundary solutions are not mislabeled as regular interior inference.
`tests/fixtures/causal_category_reference.py` reproduces independent SciPy,
statsmodels and NumPy reference fits, covariance and effects for focused regression
tests. The node help specifies actual estimator scope and statistical assumptions.

## Additional panel analyses

`panel::difference_gmm` owns one-step Arellano–Bond difference GMM with collapsed
lag-level response instruments and strictly exogenous differenced predictors.
It checks balanced consecutive period indices and identification, uses the
first-difference iid error covariance for the first-step weight, and returns
entity-score robust or conventional covariance with normal coefficient inference.
Instrument count must be below entity count. It does not claim system/two-step GMM
or supply instrument-validity tests. Matrices use the existing Linalg boundary.

`panel::fisher_unit_root` reuses individual ADF regressions and applies the matching
MacKinnon unit-root response surface. `fisher_cointegration` fits per-entity
cointegrating equations, tests their residuals without deterministic terms, and
uses the Engle–Granger response surface for the original deterministic terms and
variable count. Both combine individual p-values under cross-sectional independence,
allow different entity lengths, and reject gaps, duplicates and failed entity tests.
Input/iteration stages honor execution control; decompositions are checked at their
boundaries. Reference fixtures in `tests/fixtures/panel_category_reference.py`
reproduce statsmodels/SciPy individual and Fisher tests and an independently
optimized entity-moment GMM with general sandwich inference.

## Additional regression estimators

Shared design preparation reports absent design columns as `ShapeMismatch`,
insufficient samples as `EmptyInput`, and nonidentifiable full-rank designs as
`DataOutOfRange`. Nonfinite values produced by design scaling remain computation
failures; tuning and iteration settings retain parameter violations. Regression
estimators, parametric survival, mixed/GEE, causal, Meta and mediation-bootstrap
models and collinearity diagnostics use this same preparation contract.

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

## Longitudinal and mixed models

`longitudinal` reuses regression design scaling, coefficient inference and the
controlled optimizer. GEE supports Gaussian identity, Bernoulli logit and Poisson
log, with independence/exchangeable working correlations and uncorrected cluster
sandwich covariance. Invalid moment correlations fail rather than being clipped.
Gaussian mixed models profile fixed coefficients and residual scale in ML/REML,
estimate independent random-term variances, and return GLS covariance and BLUPs.
Nested group IDs must identify unique parents; redundant covariance designs fail.
Independent slopes retain their original zero point and have no fitted correlations.

GLMM supports a single Gaussian random intercept with Bernoulli, Poisson or NB2
response. It optimizes a per-group Laplace likelihood using standard-normal modes,
including zero-variance boundaries, and uses complete observed-information Wald
inference. NB2 estimates alpha jointly. Fixed-only predictions are inverse-link
values at zero random effects, not integrated population means. Mixed-model Wald
inference is approximate normal; no small-sample degrees-of-freedom correction is
claimed. Failed convergence and nonfinite computations remain explicit failures.

Observation, predictor and group counts have no fixed caps. Kernel workspace
admission accounts for dense Gaussian covariance separately from GEE/GLMM buffers. Input scans, iterations and likelihood/mode evaluations check
cancellation/deadlines; decompositions are checked at their boundaries. Independent
statsmodels and SciPy fixtures cover coefficients, covariance, variance components,
likelihood and predictions, with generation code in `src/longitudinal/fixtures`.

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
Input failures use Contract's shared violations: confidence/coverage and unsupported
inference/null-distribution options are `ParameterOutOfRange`; incompatible column
layouts are `ShapeMismatch`; insufficient samples are `EmptyInput`; out-of-domain
ratings and nonidentifiable coefficients are `DataOutOfRange`.
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
Negative observations and a zero overall mean report `DataOutOfRange`; empty and
nonfinite observations retain their distinct shared input violations.

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
Negative values/weights and zero weighted income report `DataOutOfRange`, independently
of shape and nonfinite-input failures.

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
The joint result also owns the two-sided 95% white-noise reference-band half-width,
using the existing normal quantile `1.959963984540054 / sqrt(n)`. Report queries and
`visualization::correlogram` consume that value; neither maintains another formula.

## OLS model boundary

Linear fit projections take intercept identity and parameter names from the
requested `constant` option; a predictor containing only ones does not change that
option. OLS/WLS/GLS and Prais construct this metadata once with the fitted result.

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
Linear fit admission reports auxiliary-data length mismatches as `ShapeMismatch`,
nonfinite weights/covariances as `NonFiniteInput`, and nonpositive WLS weights or
asymmetric GLS covariance data as `DataOutOfRange`. Unsupported GLS covariance
options remain `ParameterOutOfRange`. Direct `fit_ols` reports insufficient samples
as `EmptyInput` and nonfinite observations as `NonFiniteInput`.

`compute_cov_beta` borrows the selected `OlsCovariance` directly. It reads cluster
IDs, HAC kernels and lag settings from that selection rather than reconstructing
named parameters; OLS, WLS and IV reuse their existing design, inverse cross product
and residual buffers. IV 2SLS/LIML models consume the same `OlsOptions`, with `small`
remaining an IV-specific input. Fit projection moves those options into the result;
first-stage and postestimation checks borrow the same covariance selection.

Covariance callers supply the intercept column from the existing model or design
options. Automatic HAC bandwidth excludes that column from a multicolumn pilot
score, and includes every column when no intercept is configured; a single-column
design retains its score. WLS preserves the column's role through weighting.
The raw matrix entry accepts an explicit column index and rejects an out-of-range
index instead of guessing an intercept from column values or position.

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
Rank moments convert sample cardinalities before polynomial arithmetic. Shared
rank preparation retains only repeated-group sizes; Mann–Kendall derives its
score tie correction while compressing its existing sorted values, without a
second ranking/sort. Compression and lookup treat signed zero as one value.
Mann–Kendall uses the standard `t*(t-1)*(2*t+5)` variance adjustment and retains
zero corrected z for zero scores, including constant observations.
Categorical count-table preparation borrows labels from its owned input arrays in
one ordered index per dimension. It keeps lexical row/column order and exact cell
counts without cloning labels per observation or retaining separate level vectors.
Chi-square goodness-of-fit requires a positive observed total matching the expected
total within floating-point roundoff. Mismatched totals retain the existing data-domain
input violation rather than producing a p-value; finite-input total overflow remains
a computation failure.
Multiple-proportion inputs retain ordered success/trial pairs. Each group's success
and failure counts share one column in the two-row table passed to the shared Pearson test.
Pearson contingency tables reject a zero observed total as a data-domain violation
before dividing by it.
Mood's median test uses that same controlled Pearson owner. Its two-row table keeps
each group's above/below counts in one column, excludes pooled-median ties and retains
the original group sample sizes. Groups with no usable table information are data-domain
violations; Mood does not keep a separate chi-square implementation.
Variance-homogeneity tests retain small upper-tail probabilities instead of
subtracting a CDF near one. Levene and Brown-Forsythe use the reciprocal F
distribution with swapped degrees of freedom; Bartlett uses the chi-square
survival function. A zero F statistic retains a p-value of one.
`tests/categorical_tables.rs` covers repeated long labels whose first appearance
order differs from the table's lexical order.
For 2x2 Fisher tests, the statistic is the sample odds ratio `a*d/(b*c)`;
when `b` or `c` is zero it is absent, with the exact p-value and cell counts
retained. Zero odds with a positive denominator remain a defined zero statistic.
Exact Binomial and Poisson tails include the observed count: `Greater` uses
`P(X >= k)` and `Less` uses `P(X <= k)`. Zero counts and point-mass nulls
(probability 0/1 or rate 0) respect the selected alternative.
Count admission uses the caller's resources and execution control. Binomial
trials and Poisson events/expected counts have no additional fixed sample caps;
two-sided enumeration samples the caller's cancellation/deadline. Integer
representation bounds and finite derived values remain checked.

All four entrypoints also accept the caller's `ScientificExecutionControl`. Input,
rank/tie, table and variance scans, exact binomial/Poisson/Fisher enumeration,
Wilcoxon sign enumeration, and Mann-Kendall observations sample cancellation and
deadline during work. Sorting and distribution-library calls have checks at their
boundaries; their internals are not interruptible. The four entrypoints return
`ScientificComputationError` directly, distinguishing observation shape/domain,
nonfinite observations, invalid options, computation failures and interruption.
Kernel owns admission of retained inputs,
algorithm workspaces and reports; SCI does not introduce a second memory budget.

Binary postestimation lives in `regression::discrete::postestimation`: Logit odds
ratios transform coefficient intervals and retain the coefficient-null z test;
Logit/Probit continuous-regressor margins support AME, MEM, explicit at values and
four derivative/elasticity scales with analytic delta-method covariance. Margins
check shared execution control in observation loops; Kernel admits retained design,
quadratic workspace and cubic work before dispatch. Classifier diagnostics use
`p >= cutoff` and optional values for zero-denominator rates. Probit IRLS uses the
inverse-link derivative in its working response; default MLE covariance uses
observed information. A checked-in statsmodels 0.14.6 fixture verifies both links.
Logit/Probit fit projection borrows the model-owned response and design. IRLS
borrows its weighted matrices and vectors for cross products, then moves the
computed coefficients and covariance into the result; only the design copies
modified by row weighting are retained.

Panel fits retain the actual estimation-scale design/response/fitted/residual
sample with its transformation name and source-row groups. Estimation-scale
prediction reuses those coefficients; it does not claim to predict absorbed effects
for new entities. Selected-lag VAR fits retain exogenous labels, covariance divisor
choice and complete source rows. Prais retains the full rho iteration sequence.

`path` separates interaction fitting/probing, mediation equations and recursive path decomposition.
`preparation` owns shared centers and sample-SD probes; `effects` owns covariance contrasts and
observed-range Johnson–Neyman roots. `mediation` fits one observed mediator with optional
first/second-stage moderation. `bootstrap` retains only composite effects for paired-row
percentile inference, keeping observed probes fixed and rejecting failed replications.
`recursive` validates explicit x-index equations as a DAG, fits OLS equations and propagates
all directed-path products without enumerating paths. It does not implement latent-variable SEM.

`survey` separates weight summaries, nested stratum/PSU preparation, mean/proportion
linearization and regression score covariance. It implements single-stage with-replacement
Taylor variance; certainty-stratum handling is explicit. `regression/models/glm` owns
shared unweighted/positive-prior-weight IRLS; `likelihood` retains other likelihood models.
Survey regression replaces model information covariance with design covariance and survey
t degrees of freedom. Kish summaries describe unequal weighting only.

`power` separates model domains/sample units, test-specific noncentralities, distribution
tails and integer sample-size solving. Noncentral t/F tails sum centered Poisson/beta
series with tail-mass stopping bounds; extreme-effect shortcuts have explicit probability
bounds. Scalar planning never allocates an n-row dataset. Normal approximations are named
for proportions, Fisher-z correlations, binary-predictor logistic, Poisson rate ratios and
Schoenfeld survival designs. Equivalence/noninferiority use known-variance normal designs.
SciPy references cover tails, achieved power and minimum integer sample sizes.
