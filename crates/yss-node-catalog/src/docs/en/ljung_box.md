# Ljung–Box portmanteau test

Connect a time-ordered residual or analysis `series` with at least four non-null finite observations. `lags` ranges from 1–40, default 1; the effective order is $h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40)$.

## Hypotheses and formula

- **Null $H_0$:** $\rho_1=\cdots=\rho_h=0$; the first $h$ population autocorrelations jointly vanish.
- **Alternative $H_1$:** at least one of those autocorrelations is nonzero.

$$
Q=n(n+2)\sum_{k=1}^h\frac{\hat\rho_k^2}{n-k},
\qquad Q\overset{H_0}{\approx}\chi_h^2.
$$

$n$ is observation count and $\hat\rho_k$ the lag-$k$ sample autocorrelation after demeaning, using a full-sample sum-of-squares denominator. Lags refer to row positions.

## Outputs and interpretation

`result` contains the structured result, containing `stat=Q`, `p_value` and `lags=h`. P-values use the chi-square upper tail; $p<\alpha$ rejects joint zero correlation through lag $h$, not lag $h$ alone.

This series-only node does not adjust degrees of freedom for estimated model parameters; account for this when testing fitted residuals. Constant series fail, and non-rejection does not establish independence.

Method details: [Ljung–Box](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.acorr_ljungbox.html).
