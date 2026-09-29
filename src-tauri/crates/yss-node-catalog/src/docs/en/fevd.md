# VAR forecast-error variance decomposition

Connect a VAR Fit `model` to decompose each variable's forecast-error variance into orthogonal innovation shares. `steps` ranges from 1–1000, default 8. The model is not refitted.

## Calculation

For $K$ variables, maximum fitted lag $p$, lag matrices $A_\ell$ and innovation covariance $\Sigma$, use a lower-triangular factor $\Sigma=GG'$ in fitted variable order:

$$
\Phi_0=I_K,\qquad
\Phi_s=\sum_{\ell=1}^{\min(s,p)}A_\ell\Phi_{s-\ell},
\qquad \Theta_s=\Phi_sG.
$$

Omitted lag matrices are zero. For an actual $h$-step forecast, $h\geq1$,

$$
\Omega(h)=\sum_{s=0}^{h-1}\Theta_s\Theta_s',\qquad
\mathrm{FEVD}_{ij}(h)=
\frac{\sum_{s=0}^{h-1}(\Theta_s)_{ij}^2}{\Omega_{ii}(h)}.
$$

$\Omega_{ii}(h)$ is variable $i$'s $h$-step forecast-error variance, and the numerator is the contribution of orthogonal innovation $j$. For nondegenerate data shares lie in 0–1 and sum to one across shocks for each response variable. See [statsmodels VAR FEVD](https://www.statsmodels.org/stable/vector_ar.html#forecast-error-variance-decomposition-fevd).

## Output indexing and example

`result` and `report` are identical, containing `fevd[s][i][j]` with dimensions `(steps+1) × K × K`: **accumulation index, response variable, shock variable**. Variable order follows the fit.

**Current index $s$ accumulates $\Theta_0,\ldots,\Theta_s$, corresponding to actual forecast horizon $h=s+1$.** Thus `fevd[0]` is a one-step decomposition and `fevd[steps]` represents `steps+1` forecast steps, not a zero-step forecast at index zero.

For example, `fevd[3][0][1]=0.25` means innovation two contributes 25% of variable one's **four-step** forecast-error variance. It is not 25% of the variable's observed level or long-run change. Squared responses discard sign; consult IRF for direction.

## Conditions and limits

Cholesky recursive identification makes results order-dependent and requires substantive justification. Innovation covariance must be positive definite. Unstable VAR fits do not support the usual stable long-run interpretation, and stability is not automatically certified. If a row's accumulated variance is at most $10^{-300}$, the current convention assigns own share one and other shares zero; this is not substantive decomposition evidence.

FEVD is a postestimation decomposition with **no null/alternative hypotheses, p-values or confidence intervals**. Share size is not a significance test. These point decompositions condition on estimated parameters and exclude parameter-estimation uncertainty. Invalid fits, failed Cholesky decomposition or exceeded computation budgets fail.
