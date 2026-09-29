# Autocorrelation function (ACF)

Connect a time-ordered `series` with at least four non-null finite observations. Lags refer to row positions; the node does not read dates, sort, detrend or fill time gaps. Prepare the intended equally spaced sequence upstream.

## Parameter and formula

`lags` requests a maximum lag from 1–40, default 1. The effective maximum is

$$
h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40).
$$

For $x_1,\ldots,x_n$ with mean $\bar x$,

$$
\hat\rho_k=
\frac{\sum_{t=k+1}^{n}(x_t-\bar x)(x_{t-k}-\bar x)}
{\sum_{t=1}^{n}(x_t-\bar x)^2},
\quad k=1,\ldots,h,\qquad \hat\rho_0=1.
$$

All lags use the same full-sample sum-of-squares denominator, without an additional $n/(n-k)$ adjustment. A stable correlation interpretation generally assumes weak stationarity; trends and seasonality can induce high autocorrelation. See [statsmodels acf](https://www.statsmodels.org/stable/generated/statsmodels.tsa.stattools.acf.html).

## Outputs and interpretation

`result` and `report` are identical, containing `function=acf`, `observations=n` and `values`. `values[0]` is lag zero and `values[k]` is lag $k$; the usual length is $h+1$.

For example, lag-one ACF of 0.7 indicates substantial positive linear association between adjacent observations, not a 70% event probability. Decay and periodic peaks help explore dynamics but do not uniquely identify model order or causality.

This node **does not conduct a null/alternative hypothesis test or output confidence bands or p-values**. Use Ljung–Box to assess a joint zero-autocorrelation hypothesis. A nonzero sample correlation alone does not establish significance.

A constant series returns only `values=[1]` because its variance is zero. Omitted lags are not estimated zeros. Nulls, nonfinite inputs and numerical failures produce errors.
