# Cameron–Trivedi information-matrix decomposition

Connect an OLS or WLS `model` to diagnose error moment restrictions through heteroskedasticity, skewness, kurtosis and a total statistic. There are no parameters. The fitted sample is reused; GLS is unsupported.

## Notation and conditions

Let $n$ be sample size, $X=[\mathbf1,x_1,\ldots,x_p]$ the original design with an intercept, $u_i$ the original residuals and $s^2=\sum_i u_i^2/n$. Let $P_X$ denote least-squares projection onto the columns of $X$. Each component uses an asymptotic chi-square upper tail.

The following formulas and degrees of freedom describe the current node. A correctly specified conditional mean, independent observations, sufficiently high finite moments and adequate sample size are required. Normal homoskedastic errors provide the benchmark moment restrictions. For background, see the [Stata regress postestimation manual](https://www.stata.com/manuals/rregresspostestimation.pdf).

## Heteroskedasticity component

**Null $H_0$:** homoskedasticity; all nonconstant coefficients in the squared-residual auxiliary equation are zero.

**Alternative $H_1$:** at least one is nonzero, indicating variance related to the selected terms.

Let $Z$ contain $X$, squares of its nonconstant columns and their pairwise products. Regress $v_i=u_i^2$ on $Z$:

$$
H=n\left(1-\frac{\sum_i(v_i-\hat v_i)^2}{\sum_i(v_i-\bar v)^2}\right),
\qquad d_H=\operatorname{rank}(Z)-1.
$$

This is the node's White component. Invariant squared residuals are assigned $H=0$.

## Skewness component

**Null $H_0$:** the third-order moment restrictions $E[X_i(u_i^3-3\sigma^2u_i)]=0$ hold.

**Alternative $H_1$:** at least one of these restrictions fails.

Form $s_i=u_i^3-3s^2u_i$ and regress it on $X$, obtaining $\hat s=P_Xs$:

$$
S_{\mathrm{IM}}=n\left(1-\frac{\sum_i(s_i-\hat s_i)^2}{\sum_i s_i^2}\right),
\qquad d_S=\operatorname{rank}(X)-1.
$$

The denominator is **uncentered**, not a conventional centered $R^2$. The node reports the current $\operatorname{rank}(X)-1$ degrees-of-freedom convention. This component is not a Z transformation of sample skewness alone.

## Kurtosis component

**Null $H_0$:** $E[u_i^4-6\sigma^2u_i^2+3\sigma^4]=0$, corresponding to the normal fourth moment under zero mean and homoskedasticity.

**Alternative $H_1$:** the fourth-order restriction fails.

Form $k_i=u_i^4-6s^2u_i^2+3(s^2)^2$ and regress it on a constant:

$$
K_{\mathrm{IM}}=n\left(1-\frac{\sum_i(k_i-\bar k)^2}{\sum_i k_i^2}\right),
\qquad d_K=1.
$$

A zero denominator is assigned a zero kurtosis statistic.

## Joint test

**Null $H_0$:** all three groups of moment restrictions hold.

**Alternative $H_1$:** at least one group fails.

$$
IM=H+S_{\mathrm{IM}}+K_{\mathrm{IM}},\qquad d=d_H+d_S+1.
$$

For each statistic $T$, use its degrees of freedom $d_T$ to compute $p_T=1-F_{\chi^2_{d_T}}(T)$. The total relies on the asymptotic decomposition under the null; it does not add p-values or establish all distributional properties.

## WLS, outputs and interpretation

WLS replaces only the heteroskedasticity component with White's test on $v_i=w_i u_i^2$. Skewness and kurtosis still use **unweighted original residuals** and $s^2=\sum_i u_i^2/n$. This is not a complete IM test applied to whitened residuals.

`result` and `report` are identical, with `test=information_matrix`. Inner `result` contains `heteroskedasticity`, `skewness`, `kurtosis` and `total`, each with `chi2`, `df` and `p_value`.

For example, a total p-value below a preselected 0.05 together with a prominent heteroskedasticity component motivates checking variance specification. For each component, $p<\alpha$ rejects its own null; non-rejection only indicates insufficient evidence. Components help locate problems but are not independent, multiplicity-adjusted findings.

The sample must satisfy White's full-expansion threshold, and the design must contain an intercept and at least one predictor. Zero degrees of freedom, degenerate third-order auxiliary quantities, estimation failures or nonfinite computations can fail the whole node. High residual powers amplify outlier effects.
