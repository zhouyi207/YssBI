# VAR orthogonalized impulse responses

Connect a VAR Fit `model` to compute each variable's response to an orthogonal innovation shock. `steps` ranges from 1–1000, default 8. Results include horizons zero through `steps`, without refitting.

## Calculation

Let $p$ be the largest fitted lag, $A_1,\ldots,A_p$ the lag matrices with omitted lags set to zero, and $\Sigma$ the innovation covariance. Compute moving-average coefficients:

$$
\Phi_0=I_K,\qquad
\Phi_s=\sum_{\ell=1}^{\min(s,p)}A_\ell\Phi_{s-\ell}.
$$

Use a lower-triangular Cholesky factor to orthogonalize:

$$
\Sigma=GG',\qquad u_t=G\varepsilon_t,\quad
\operatorname{Var}(\varepsilon_t)=I_K,\qquad
\Theta_s=\Phi_sG.
$$

$K$ is the number of variables. $(\Theta_s)_{ij}$ is variable $i$'s response at horizon $s$ to a one-standard-deviation shock in unit-variance orthogonal innovation $j$, measured in the response variable's units. Responses are not cumulatively summed. See [statsmodels VAR impulse responses](https://www.statsmodels.org/stable/vector_ar.html#impulse-response-analysis).

## Ordering and identification

The lower-triangular factor follows fitted variable order. Later orthogonal shocks have zero contemporaneous effect on earlier variables; changing order generally changes responses. This recursive identification needs substantive justification. Orthogonalization alone does not establish economic causality. Deterministic terms and exogenous paths are held fixed; responses use only dynamic coefficients and innovation covariance.

## Outputs and example

`result` and `report` are identical, containing `oirf[s][i][j]` with dimensions `(steps+1) × K × K`: **horizon, response variable, shock variable**. Names follow the source model's order. `oirf[0]` equals $G$, not the identity matrix.

For example, `oirf[2][0][1]=0.3` means a positive one-standard-deviation shock to the second orthogonal innovation raises the first variable by 0.3 of its units two periods later. It is neither a 30% probability nor automatically a percentage change.

This is a postestimation response function with **no null/alternative hypothesis or p-value**. The node provides no confidence intervals, significance tests or bootstrap bands; a positive point estimate alone does not establish a significantly positive response.

Innovation covariance must be positive definite for Cholesky decomposition, otherwise calculation fails. Stable VAR dynamics support the usual decay and long-run interpretation. Stability is not automatically certified; an unstable fit may yield growing finite-horizon responses. Large horizons and dimensions are also subject to computation budgets.
