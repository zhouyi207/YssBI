# VAR orthogonalized impulse responses

Connect a VAR Fit `model` to compute each variable's period-by-period response to a one-standard-deviation orthogonal innovation shock. `steps` ranges from 1–1000, default 8, with horizons zero through `steps`.

## Formula

$$
\Phi_0=I,\qquad
\Phi_s=\sum_{\ell=1}^{\min(s,p)}A_\ell\Phi_{s-\ell},
\qquad \Sigma=GG',\qquad \Theta_s=\Phi_sG.
$$

$A_\ell$ are fitted VAR lag matrices, $p$ the maximum lag, and omitted lags are zero. $G$ is the lower-triangular Cholesky factor of innovation covariance $\Sigma$; $\Theta_s$ is the orthogonal response at horizon $s$.

## Outputs and interpretation

`result` and `report` are identical. `oirf[s][i][j]` indexes **horizon, response variable, shock variable**, with dimensions `(steps+1) × K × K` for $K$ variables. Horizon zero is the impact period, and `oirf[0]` equals $G$.

Responses are in the response variable's units and are not cumulative. Ordering follows the fitted model and generally affects orthogonal responses. Innovation covariance must be positive definite.

Outputs are point estimates without confidence intervals or p-values. VAR stability is not automatically checked.

Method details: [VAR impulse responses](https://www.statsmodels.org/stable/vector_ar.html#impulse-response-analysis).
