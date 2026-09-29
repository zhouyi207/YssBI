# Linear coefficient t / Wald tests

Connect a fitted linear `model` and specify `hypothesis`, default `x1 = 0`, up to 4096 characters. Use the actual coefficient names from the model report. The existing estimates, covariance and residual degrees of freedom are reused without refitting.

## Single-constraint t test

Examples include `x1 = 0`, `x1 = x2` and `2*x1 + x2 = 1`. Let $\hat\beta$ be the coefficient vector, $V$ its covariance, $R$ a constraint row and $r$ a scalar target.

**Null $H_0$:** $R\beta=r$.

**Alternative $H_1$:** $R\beta\ne r$; a single equality is two-sided.

$$
t=\frac{R\hat\beta-r}{\sqrt{RVR'}},
\qquad t\overset{H_0}{\sim}t_\nu,\qquad
p=2[1-F_{t_\nu}(|t|)].
$$

$\nu$ is the source model's residual degrees of freedom. A single inequality specifies a directional alternative:

| Input                 | Null           | Alternative | p-value          |
| --------------------- | -------------- | ----------- | ---------------- |
| `x1 > 0` or `x1 >= 0` | $\beta_1\leq0$ | $\beta_1>0$ | $1-F_{t_\nu}(t)$ |
| `x1 < 0` or `x1 <= 0` | $\beta_1\geq0$ | $\beta_1<0$ | $F_{t_\nu}(t)$   |

Inequalities are calibrated at their equality boundary. The entered direction is the alternative to investigate. Less-than expressions are internally sign-reversed, as are the reported `stat` and `r_beta_minus_r`; read `h0_form`, `h1_form` and `alternative` alongside them.

## Joint Wald F test

Separate joint equalities with commas, for example `x1 = 0, x2 = 0`. Let $R$ be a $q\times k$ matrix, $r$ a $q$-vector and $q$ the number of independent restrictions.

**Null $H_0$:** every equality in $R\beta=r$ holds.

**Alternative $H_1$:** at least one equality fails.

$$
W=(R\hat\beta-r)'(RVR')^{-1}(R\hat\beta-r),
\qquad F=\frac Wq,
$$

$$
F\overset{H_0}{\sim}F_{q,\nu},\qquad
p=1-F_{F_{q,\nu}}(F).
$$

**The output `stat` is $F=W/q$, not $W$ or a chi-square statistic.** Joint tests support equalities only. Redundant constraints causing a singular $RVR'$ fail; degrees of freedom are not automatically reduced. See [statsmodels wald_test](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.RegressionResults.wald_test.html).

## Outputs, conditions and example

`result` and `report` are identical, containing `test_type` (`t` / `wald`), `h0_form`, `h1_form`, `alternative`, `stat`, `df1`, `df2`, `p_value` and `r_beta_minus_r`. A t test has `df1=1`, `df2=ν`; Wald has `df1=q`, `df2=ν`. For joint constraints, `r_beta_minus_r` is a zero placeholder, not evidence that every contrast is zero.

For example, rejecting `x1 = 0, x2 = 0` at a preselected 5% level means at least one coefficient differs from zero, not that both do. Non-rejection is insufficient evidence, not proof of exact equality.

Classical homoskedastic normal linear models provide exact t/F calibration. With robust, clustered or other upstream covariance, the node reuses that $V$ and the model's $\nu$; interpret inference under the corresponding approximation. Positive residual degrees of freedom, valid constraint covariance and linear expressions are required. Unknown names, nonlinear coefficient products, joint/mixed inequalities or nonfinite results fail.
