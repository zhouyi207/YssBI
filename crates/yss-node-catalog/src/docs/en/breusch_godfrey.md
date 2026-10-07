# Breusch–Godfrey serial-correlation test

Connect a linear `model` to test residual serial correlation conditional on the original regressors. Fitted rows should be in time order, with at least four observations.

| Parameter    | Default | Meaning                                                                         |
| ------------ | ------- | ------------------------------------------------------------------------------- |
| `lags`       | 1       | Requested order 1–40; effective $h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40)$ |
| `bg_nomiss0` | `true`  | Zero-fill unavailable initial residual lags; disable to drop the first $h$ rows |

## Hypotheses and formula

- **Null $H_0$:** $\rho_1=\cdots=\rho_h=0$; all error serial-correlation coefficients through lag $h$ vanish.
- **Alternative $H_1$:** at least one coefficient is nonzero.

The auxiliary regression uses original residuals $u_t$ and design rows $X_t'$:

$$
u_t=X_t'\gamma+\sum_{j=1}^h\rho_j u_{t-j}+v_t,
\qquad LM=n_{\mathrm{aux}}R_{\mathrm{aux}}^2
\overset{H_0}{\approx}\chi_h^2.
$$

$R_{\mathrm{aux}}^2$ uses the uncentered denominator $\sum u_t^2$. Auxiliary sample size is $n_{\mathrm{aux}}=n$ with zero filling and $n-h$ when initial rows are dropped.

## Outputs and interpretation

`result` contains the structured result, containing `stat=LM`, `p_value` and `lags=h`. P-values use the chi-square upper tail; $p<\alpha$ rejects joint zero serial correlation through lag $h$.

WLS/GLS inputs still use an unweighted auxiliary regression on original residuals and design, without automatic weighting or whitening. Dropping initial rows additionally requires $n-h>k+h$, where $k$ is the design column count. Singular auxiliary designs prevent calculation.

Method details: [Breusch–Godfrey](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.acorr_breusch_godfrey.html).
