# IV Hausman endogeneity test

Connect an IV 2SLS `model` fitted with `nonrobust` covariance. It compares IV and OLS coefficients on the same sample and structural equation. There are no parameters. The fitted IV estimates are reused and an OLS comparator is calculated. This is not a panel FE/RE comparison, nor an output of the separate Durbin or Wu–Hausman variants.

## Hypotheses

**Null $H_0$:** the designated endogenous regressors are actually exogenous; both OLS and IV are consistent, and OLS is efficient under the required homoskedasticity and other conditions.

**Alternative $H_1$:** at least one tested regressor is endogenous; OLS is inconsistent while IV remains consistent provided instruments are valid and identification holds.

Instrument relevance and exogeneity are prerequisites, not conclusions of this test. Hausman is not an instrument-validity or weak-instrument test.

## Calculation

Let $X$ be the structural design including any fitted intercept, $Z$ the instrument design including exogenous regressors and excluded instruments, $n$ the sample size and $k$ the number of structural columns:

$$
P_Z=Z(Z'Z)^{-1}Z',\quad \hat X=P_ZX,\quad
\hat\beta_{\mathrm{OLS}}=(X'X)^{-1}X'y,\quad
s_{\mathrm{OLS}}^2=\frac{\sum_i(y_i-X_i'\hat\beta_{\mathrm{OLS}})^2}{n-k}.
$$

The node uses a common OLS disturbance variance, the `sigmamore` convention:

$$
V_{\mathrm{IV}}=s_{\mathrm{OLS}}^2(\hat X'\hat X)^{-1},\qquad
V_{\mathrm{OLS}}=s_{\mathrm{OLS}}^2(X'X)^{-1},
$$

$$
\Delta=\hat\beta_{\mathrm{IV}}-\hat\beta_{\mathrm{OLS}},\quad
A=V_{\mathrm{IV}}-V_{\mathrm{OLS}},\quad
H=\Delta'A^+\Delta,\quad q=\operatorname{rank}(A),
$$

$$
H\overset{H_0}{\approx}\chi_q^2,\qquad
p=1-F_{\chi_q^2}(H).
$$

$A^+$ is an SVD Moore–Penrose inverse. All retained structural coefficients, including any intercept, participate; degrees of freedom use the effective rank of the covariance difference, not a hardcoded endogenous-variable count. Negative numerical statistics are clipped to zero. See the [Stata Hausman manual](https://www.stata.com/manuals/rhausman.pdf) for common-variance and intercept conventions.

## Outputs and interpretation

`result` and `report` are identical, containing `hausman` with `stat`, `df` and `p_value`. For example, $p=0.02$ at a preselected 5% level rejects exogeneity, conditional on valid, sufficiently relevant instruments and suitable specification.

Non-rejection does not establish exogeneity; weak instruments or small samples can reduce power. Robust/clustered covariance, unavailable OLS comparison, zero effective rank, degenerate variance or nonfinite results prevent a valid test and fail the node.
