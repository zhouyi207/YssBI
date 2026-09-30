# Cameron–Trivedi information-matrix decomposition

Connect an OLS or WLS `model` to report heteroskedasticity, skewness, kurtosis and joint tests. There are no parameters. GLS is unsupported; the model should include an intercept and at least one predictor.

Below, $n$ is sample size, $X$ the original design, $u_i$ residuals and $s^2=\sum_i u_i^2/n$.

## Heteroskedasticity component

- **Null $H_0$:** homoskedasticity; all nonconstant squared-residual auxiliary coefficients are zero.
- **Alternative $H_1$:** at least one coefficient is nonzero.

Use White's auxiliary regression of $u_i^2$ on $Z$, comprising predictors, squares and cross-products.

$$
H=nR_{\mathrm{aux}}^2,\qquad d_H=\operatorname{rank}(Z)-1.
$$

## Skewness component

- **Null $H_0$:** the third-order restrictions $E[X_i(u_i^3-3\sigma^2u_i)]=0$ hold.
- **Alternative $H_1$:** at least one restriction fails.

Regress $s_i=u_i^3-3s^2u_i$ on $X$, with fitted values $\hat s_i$:

$$
S_{\mathrm{IM}}=n\left(1-\frac{\sum_i(s_i-\hat s_i)^2}{\sum_i s_i^2}\right),
\qquad d_S=\operatorname{rank}(X)-1.
$$

## Kurtosis component

- **Null $H_0$:** the fourth-order restriction $E[u_i^4-6\sigma^2u_i^2+3\sigma^4]=0$ holds.
- **Alternative $H_1$:** that restriction fails.

Let $k_i=u_i^4-6s^2u_i^2+3(s^2)^2$ and regress it on a constant:

$$
K_{\mathrm{IM}}=n\left(1-\frac{\sum_i(k_i-\bar k)^2}{\sum_i k_i^2}\right),
\qquad d_K=1.
$$

## Joint test

- **Null $H_0$:** all three components' moment restrictions hold.
- **Alternative $H_1$:** at least one component's restrictions fail.

$$
IM=H+S_{\mathrm{IM}}+K_{\mathrm{IM}},\qquad d=d_H+d_S+1.
$$

Each uses an asymptotic chi-square upper-tail p-value with its corresponding degrees of freedom. Reject that null when $p<\alpha$. The formulas reflect this node's degrees-of-freedom conventions.

## Outputs and usage

`result` contains the structured result: `test=information_matrix` with inner `result` components `heteroskedasticity`, `skewness`, `kurtosis` and `total`, each containing `chi2`, `df` and `p_value`.

WLS replaces only the heteroskedasticity component with White's test on $w_i u_i^2$; skewness and kurtosis retain unweighted original residuals. The sample must meet White's full-expansion requirement. Components help locate problems and receive no multiplicity correction.

Method details: [Stata IM decomposition](https://www.stata.com/manuals/rregresspostestimation.pdf).
