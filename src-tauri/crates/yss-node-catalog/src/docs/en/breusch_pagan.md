# Breusch–Pagan / Koenker heteroskedasticity test

Tests whether regression error variance changes systematically with selected variables. Connect an OLS or WLS `model`. The node reuses fitted observations, residuals $u_i$, fitted values $\hat y_i$ and design $X$, without refitting the original model. GLS is unsupported.

## Parameters and auxiliary design

| Parameter | Default | Meaning                                                                |
| --------- | ------- | ---------------------------------------------------------------------- |
| `rhs`     | `false` | Use $Z=[\mathbf1,\hat y]$; when enabled, use the original design $Z=X$ |
| `koenker` | `false` | Original BP; enable for the studentized Koenker form                   |

The standard interpretation below requires an intercept. RHS mode does not add one and always uses $q=\operatorname{cols}(Z)-1$ degrees of freedom; standard BP inference should not be applied directly to an intercept-free fit.

## Original Breusch–Pagan

**Null $H_0$:** $\operatorname{Var}(u_i\mid Z_i)=\sigma^2$; all nonconstant coefficients in the auxiliary variance equation are zero.

**Alternative $H_1$:** variance changes with at least one nonconstant term in $Z$.

For OLS, define

$$
\hat\sigma^2=\frac1n\sum_i u_i^2,\quad
g_i=\frac{u_i^2}{\hat\sigma^2},\quad
\hat g=Z(Z'Z)^{-1}Z'g.
$$

With $\mathrm{TSS}=\sum_i(g_i-1)^2$ and $\mathrm{RSS}=\sum_i(g_i-\hat g_i)^2$,

$$
LM_{\mathrm{BP}}=\tfrac12(\mathrm{TSS}-\mathrm{RSS})
\overset{H_0}{\approx}\chi_q^2,\qquad
p=1-F_{\chi_q^2}(LM_{\mathrm{BP}}).
$$

$n$ is the fitted sample size. The default has $q=1$; RHS mode uses the number of design columns minus one. Original BP calibration assumes normal errors and independent observations, among other regularity conditions.

## Studentized Koenker

**Null $H_0$:** homoskedasticity; all nonconstant coefficients of the chosen variance equation are zero.

**Alternative $H_1$:** at least one is nonzero, indicating variance explained by the chosen variables.

Using the same $g$ and $Z$,

$$
R_{\mathrm{aux}}^2=1-\frac{\mathrm{RSS}}{\mathrm{TSS}},\qquad
LM_{\mathrm K}=nR_{\mathrm{aux}}^2
\overset{H_0}{\approx}\chi_q^2,\qquad
p=1-F_{\chi_q^2}(LM_{\mathrm K}).
$$

This form relaxes error normality but still needs independence and suitable moment conditions. It does not automatically handle serial correlation or clustering. See the [statsmodels BP/Koenker documentation](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.het_breuschpagan.html).

## WLS convention

Retain the fit's positive precision weights $w_i$. Let $W=\operatorname{diag}(w_i)$ and $\hat\sigma_w^2=\sum_iw_i u_i^2/n$. Weighted BP regresses $f_i=u_i^2/\hat\sigma_w^2-1$ on $Z$:

$$
\hat f=Z(Z'WZ)^{-1}Z'Wf,\qquad
LM_{\mathrm{BP},w}=\tfrac12\sum_iw_i\hat f_i^2.
$$

Koenker applies the same weighted auxiliary regression to $g_i=u_i^2/\hat\sigma_w^2$ and computes

$$
\bar g_w=\frac{\sum_iw_i g_i}{\sum_iw_i},\quad
R_w^2=1-\frac{\sum_iw_i(g_i-\hat g_i)^2}
{\sum_iw_i(g_i-\bar g_w)^2},\qquad LM_{\mathrm K,w}=nR_w^2.
$$

Degrees of freedom and upper-tail p-values follow the rules above. These are the node's current weighted conventions; weights are not replication counts. Interpret them in the context of the fit's precision weights and variance specification.

## Outputs, interpretation and limits

`result` and `report` are identical: `test=breusch_pagan` with inner `result` fields `lm_stat`, `df` and `p_value`. Only the selected variant runs.

With a preselected $\alpha$, $p<\alpha$ rejects homoskedasticity; otherwise there is insufficient evidence for the selected variance equation. For example, the default tests variance related to the fitted mean, while `rhs=true` examines the explanatory variables. Rejection does not identify the exact variance function; non-rejection does not rule out patterns omitted from $Z$.

Default mode requires at least four observations; RHS mode requires $n\geq k+2$, where $k$ is the design column count. Zero residual variance, singular auxiliary designs and nonfinite computations fail. Constant fitted values can make the default design singular. If squared residuals have no variation, Koenker assigns auxiliary $R^2=0$; this does not prove homoskedasticity. A robust covariance choice in the upstream model does not change these LM formulas.
