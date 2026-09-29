# Breusch–Godfrey serial-correlation test

Connect a fitted linear `model` to assess residual serial correlation through a selected order, conditional on the original regressors. The retained residuals and design are reused without refitting the response model. Fitted row order must represent the intended time order.

## Parameters and hypotheses

| Parameter    | Default | Meaning                                                                                                |
| ------------ | ------- | ------------------------------------------------------------------------------------------------------ |
| `lags`       | 1       | Requested order 1–40; effective $h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40)$                        |
| `bg_nomiss0` | `true`  | Fill unavailable initial residual lags with zero and keep $n$ rows; disable to drop the first $h$ rows |

**Null $H_0$:** $\rho_1=\cdots=\rho_h=0$, no error serial correlation through lag $h$ conditional on $X_t$.

**Alternative $H_1$:** at least one $\rho_j\ne0$.

## Auxiliary regression and statistic

Regress fitted residuals as

$$
u_t=X_t'\gamma+\sum_{j=1}^h\rho_j u_{t-j}+v_t.
$$

$X_t'$ is row $t$ of the original design, including any existing intercept; none is added. For auxiliary sample $\mathcal I$ of size $n_{\mathrm{aux}}$, the current calculation uses an uncentered coefficient of determination:

$$
R_{\mathrm{aux}}^2
=1-\frac{\sum_{t\in\mathcal I}\hat v_t^2}{\sum_{t\in\mathcal I}u_t^2},
\qquad LM=n_{\mathrm{aux}}R_{\mathrm{aux}}^2,
$$

$$
LM\overset{H_0}{\approx}\chi_h^2,\qquad
p=1-F_{\chi_h^2}(LM).
$$

Zero filling uses $n_{\mathrm{aux}}=n$; dropping initial rows uses $n-h$. A denominator at most $10^{-20}$ is assigned $R^2=0$, which is degenerate rather than valid ordinary inference. See [statsmodels Breusch–Godfrey](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.acorr_breusch_godfrey.html).

## Outputs and interpretation

`result` and `report` are identical, containing `stat=LM`, `p_value` and `lags=h`. Only the LM chi-square form is returned, not an auxiliary F test.

For example, at `lags=4`, a p-value below a preselected 0.05 rejects the joint exclusion of the first four residual lags. It does not identify lag four alone. Non-rejection neither proves independence nor rules out correlation beyond the selected lags.

Classical BG needs suitable dynamic mean specification, error moment conditions and sample size. Lagged dependent variables can be included under the appropriate exogeneity conditions. Currently even WLS/GLS inputs use an **unweighted auxiliary regression on original residuals and design**, without weighting, whitening or robust covariance; this is not a weighted or robust BG procedure.

At least four observations are required. Dropping initial rows additionally requires $n-h>k+h$, where $k$ is the original design column count. Singular auxiliary designs, unavailable calculations and nonfinite results fail. No time intervals are inferred; unsorted or concatenated group data change the meaning of the test.
