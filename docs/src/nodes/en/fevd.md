# VAR forecast-error variance decomposition

Connect a VAR Fit `model` to decompose forecast-error variance into orthogonal innovation shares. `steps` ranges from 1–1000, default 8.

## Formula

$$
\mathrm{FEVD}_{ij}(h)=
\frac{\sum_{s=0}^{h-1}(\Theta_s)_{ij}^2}
{\sum_{s=0}^{h-1}\sum_{j=1}^{K}(\Theta_s)_{ij}^2}.
$$

$\Theta_s$ is the VAR orthogonal impulse-response matrix at horizon $s$, $K$ the variable count and $h$ the forecast horizon. The result is shock $j$'s share of variable $i$'s forecast-error variance. Shares across shocks normally sum to one.

## Outputs and interpretation

`result` contains the structured result. `fevd[s][i][j]` indexes **accumulation index, response variable, shock variable**, with dimensions `(steps+1) × K × K`.

**Array index $s$ corresponds to forecast horizon $h=s+1$**: `fevd[0]` is a one-step decomposition and `fevd[steps]` represents `steps+1` forecast steps. Values are proportions: 0.25 means 25%. Consult IRF for response direction.

Orthogonalization uses Cholesky decomposition in fitted variable order, making shares order-dependent. Innovation covariance must be positive definite. No confidence intervals or p-values are provided, and model stability is not automatically checked.

Method details: [VAR FEVD](https://www.statsmodels.org/stable/vector_ar.html#forecast-error-variance-decomposition-fevd).
