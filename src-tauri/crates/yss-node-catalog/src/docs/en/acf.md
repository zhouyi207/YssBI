# Autocorrelation function (ACF)

Connect a time-ordered `series` with at least four non-null finite observations. `lags` ranges from 1–40, default 1; the effective maximum is $h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40)$.

## Formula

$$
\hat\rho_k=
\frac{\sum_{t=k+1}^{n}(x_t-\bar x)(x_{t-k}-\bar x)}
{\sum_{t=1}^{n}(x_t-\bar x)^2},
\quad k=1,\ldots,h,\qquad \hat\rho_0=1.
$$

$n$ is observation count and $\bar x$ the series mean. All lags use the full-sample denominator, without an $n/(n-k)$ correction. Lags refer to row positions; prepare sorting and time spacing upstream.

## Outputs and interpretation

`result` and `report` are identical, containing `function=acf`, `observations=n` and `values`. The array starts at lag zero and normally has length $h+1$. A constant series returns only `[1]`.

Positive values indicate same-direction linear association and negative values opposite-direction association. No confidence bands or p-values are output. Use Ljung–Box for a joint zero-autocorrelation test.

Method details: [ACF](https://www.statsmodels.org/stable/generated/statsmodels.tsa.stattools.acf.html).
