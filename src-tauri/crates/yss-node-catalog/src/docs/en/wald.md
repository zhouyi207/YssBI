# Linear coefficient t / Wald tests

Connect a linear `model` and enter coefficient constraints in `hypothesis`, default `x1 = 0`, up to 4096 characters. Names follow the model report; estimates, covariance and residual degrees of freedom are reused.

## Single-constraint t test

Supports linear expressions such as `x1 = 0`, `x1 = x2` and `2*x1 + x2 = 1`.

- **Null $H_0$:** $R\beta=r$.
- **Alternative $H_1$:** $R\beta\ne r$.

$$
t=\frac{R\hat\beta-r}{\sqrt{RVR'}},\qquad
t\overset{H_0}{\sim}t_\nu,\qquad
p=2[1-F_{t_\nu}(|t|)].
$$

$\hat\beta$, $V$ and $\nu$ are the model estimates, covariance and residual degrees of freedom; $R\beta=r$ represents the constraint. A single inequality selects a directional alternative:

| Input                 | Null           | Alternative | p-value          |
| --------------------- | -------------- | ----------- | ---------------- |
| `x1 > 0` or `x1 >= 0` | $\beta_1\leq0$ | $\beta_1>0$ | $1-F_{t_\nu}(t)$ |
| `x1 < 0` or `x1 <= 0` | $\beta_1\geq0$ | $\beta_1<0$ | $F_{t_\nu}(t)$   |

Less-than output statistics are sign-reversed; read them with `h0_form`, `h1_form` and `alternative`.

## Joint Wald F test

Separate equalities with commas, for example `x1 = 0, x2 = 0`.

- **Null $H_0$:** all equalities in $R\beta=r$ hold.
- **Alternative $H_1$:** at least one equality fails.

$$
F=\frac{(R\hat\beta-r)'(RVR')^{-1}(R\hat\beta-r)}q,
\qquad F\overset{H_0}{\sim}F_{q,\nu}.
$$

$q$ is the constraint count. P-values use the F upper tail. Joint tests support equalities only; redundant or nonlinear constraints prevent calculation.

## Outputs and interpretation

`result` contains the structured result, containing `test_type`, `h0_form`, `h1_form`, `alternative`, `stat`, `df1`, `df2`, `p_value` and `r_beta_minus_r`. Wald `stat` is an **F statistic**. For joint constraints, `r_beta_minus_r` is a zero placeholder.

Reject the corresponding null when $p<\alpha$. Joint rejection means at least one restriction fails. Robust or clustered upstream fits retain their $V$ and $\nu$ and require the corresponding approximate interpretation.

Method details: [Linear Wald tests](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.RegressionResults.wald_test.html).
