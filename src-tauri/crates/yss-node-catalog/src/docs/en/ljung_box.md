# Ljung–Box portmanteau test

Connect a time-ordered residual or analysis `series` with at least four non-null finite observations. Lags use row positions; sorting, detrending and gap filling are not automatic.

## Parameter and hypotheses

`lags` ranges from 1–40, default 1. The effective order is

$$
h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40).
$$

**Null $H_0$:** $\rho_1=\cdots=\rho_h=0$; the first $h$ population autocorrelations jointly vanish.

**Alternative $H_1$:** at least one $\rho_k\ne0$ for $1\leq k\leq h$.

Here a “white-noise test” assesses these correlation restrictions, not a complete proof of independence, normality or zero correlation at every lag.

## Calculation

For $x_1,\ldots,x_n$, demean using the full-sample mean $\bar x$:

$$
\hat\rho_k=
\frac{\sum_{t=k+1}^n(x_t-\bar x)(x_{t-k}-\bar x)}
{\sum_{t=1}^n(x_t-\bar x)^2},\qquad
Q=n(n+2)\sum_{k=1}^h\frac{\hat\rho_k^2}{n-k}.
$$

The current node uses

$$
Q\overset{H_0}{\approx}\chi_h^2,\qquad
p=1-F_{\chi_h^2}(Q).
$$

This is a joint statistic, not a test of lag $h$ alone. See [statsmodels Ljung–Box](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.acorr_ljungbox.html), including its discussion of model degrees of freedom.

## Outputs and interpretation

`result` and `report` are identical, containing `stat=Q`, `p_value` and `lags=h`. Only one joint result at the effective order is returned, not a table at every lag.

For example, requesting `lags=10` with $n=12$ uses $h=5$ and a $\chi^2_5$ reference. At a preselected $\alpha$, $p<\alpha$ rejects zero correlation through lag five; non-rejection does not establish independence or correct model specification.

The series-only node receives no parameter count and **does not adjust model degrees of freedom**. A common adjustment for fitted ARMA residuals uses $h-p_{\mathrm{AR}}-q_{\mathrm{MA}}$; it is not applied here. Establish an approximately stationary, equally spaced series and consider its estimation context before relying on the asymptotic p-value.

Constant series have undefined sample autocorrelations and fail. Too few observations, nulls, nonfinite inputs and nonfinite results also fail.
