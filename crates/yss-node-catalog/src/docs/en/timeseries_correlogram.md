# Correlogram: ACF and PACF

Connect a finite, equally spaced numeric `series` in time order, with at least four observations and nonzero variation. Missing values are rejected. `ts_maximum_lag=20` selects 1–40 displayed lags; the actual display is limited further to floor(n/2) − 1. This is a lag-display policy, not a data-row ceiling: all observations enter the correlations.

$$
\widehat\rho_k=\frac{\sum_{t=k+1}^n(y_t-\bar y)(y_{t-k}-\bar y)}{\sum_{t=1}^n(y_t-\bar y)^2},\qquad
Q(h)=n(n+2)\sum_{k=1}^{h}\frac{\widehat\rho_k^2}{n-k}.
$$

ACF uses the common full-sample variance denominator; PACF uses the Durbin–Levinson/Yule–Walker recursion. The displayed pointwise 95% white-noise reference band is ±1.96/sqrt(n), not a simultaneous interval. For the Ljung–Box row at lag h, $H_0: \rho_1=\cdots=\rho_h=0$ against any nonzero autocorrelation, with an approximate chi-square reference on h degrees of freedom. The node applies no fitted-model degrees-of-freedom correction; residual-model diagnostics require the appropriate adjusted test.

`result` contains `acf`, `pacf`, `ciHalfWidth` and `n`. Each plotted entry carries `lag` and `value`; ACF entries also contain `qStat` and `pValue`, while PACF entries leave those tests null. The existing correlogram renderer shows both correlation sequences. Strong correlations can reflect trend or seasonality; the plot alone does not establish a particular ARIMA order.
