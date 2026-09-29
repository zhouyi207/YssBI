# Partial autocorrelation function (PACF)

Connect a time-ordered `series` with at least four finite, non-null observations. PACF measures linear association with a lag after accounting for intermediate lags. Row order defines time; sorting and gap filling are not automatic.

## Parameter and calculation

`lags` ranges from 1–40, default 1. The effective maximum is $h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40)$. First compute ACF without a lag-dependent sample-size correction:

$$
\hat\rho_k=
\frac{\sum_{t=k+1}^{n}(x_t-\bar x)(x_{t-k}-\bar x)}
{\sum_{t=1}^{n}(x_t-\bar x)^2}.
$$

Then apply the Yule–Walker / Durbin–Levinson recursion:

$$
\phi_{1,1}=\hat\rho_1,\qquad
\phi_{k,k}=
\frac{\hat\rho_k-\sum_{j=1}^{k-1}\phi_{k-1,j}\hat\rho_{k-j}}
{1-\sum_{j=1}^{k-1}\phi_{k-1,j}\hat\rho_j},
$$

$$
\phi_{k,j}=\phi_{k-1,j}-\phi_{k,k}\phi_{k-1,k-j},
\quad j=1,\ldots,k-1,\qquad
\widehat{\mathrm{PACF}}(k)=\phi_{k,k}.
$$

$\phi_{k,j}$ is the $j$th coefficient in the order-$k$ linear prediction recursion. This is neither Burg estimation nor separate OLS regressions on changing samples. See [statsmodels levinson_durbin](https://www.statsmodels.org/stable/generated/statsmodels.tsa.stattools.levinson_durbin.html).

## Outputs and interpretation

`result` and `report` are identical, containing `function=pacf`, `observations=n` and `values`. The array **starts at lag one**: `values[0]` is $\phi_{1,1}$. It normally has length $h$ and contains no lag-zero entry.

For example, a lag-two PACF near zero indicates little additional sample linear association after accounting for lag one. PACF cutoffs can guide order selection for stationary AR models, but sampling variation, trends and seasonality can obscure this pattern.

This node **does not output null/alternative hypothesis tests, p-values or confidence bands**. Near zero is not a formal non-significance finding. Establish an appropriate equally spaced, stationary analysis series first.

A constant series returns an empty array because lagged PACFs are undefined. A nonpositive recursion denominator, invalid numerical correlation, nulls or nonfinite inputs fail instead of being replaced with zeros.
