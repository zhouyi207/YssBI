# SARIMA forecast

Fit a univariate SARIMA model by conditional least squares and forecast on the original observation scale.

## Input and parameters

Connect one finite numeric `series`, already ordered and equally spaced. Nulls are rejected; rows are neither sorted nor dropped. There is no fixed row-count ceiling.

`ts_p=1`, `ts_d=1`, `ts_q=0` specify nonnegative AR, difference and MA orders. Seasonal orders `ts_seasonal_p=0`, `ts_seasonal_d=1`, `ts_seasonal_q=1` are nonnegative integers; `ts_period=12` is an integer at least 2. `constant=true` estimates the mean of the differenced series: a level mean without differencing, a drift for first differences, and an integrated deterministic component for higher differencing. `ts_horizon=10` is a positive integer. `ts_confidence=0.95` lies strictly between 0 and 1. `max_iterations=500` is positive; `tolerance=0.000001` lies in $[10^{-12},0.01]$.

## Estimation

$$
\phi(B)\Phi(B^m)(w_t-\mu)=\theta(B)\Theta(B^m)\varepsilon_t,\qquad w_t=(1-B)^d(1-B^m)^D y_t.
$$

$B$ is the lag operator, $\mu$ the differenced mean and $\varepsilon_t$ the innovation. The AR polynomial uses minus signs and the MA polynomial plus signs. Seasonal factors multiply ordinary factors. Stationarity and invertibility are imposed on fitted AR/MA factors. Presample errors are zero; the first maximum AR/MA lag after differencing is excluded from the objective. The remaining sample must exceed the number of estimated mean/AR/MA coefficients, with positive innovation variance. Orders are selected explicitly; this node does not search over orders.

## Results

`parameters` reports AR/MA coefficients, the optional differenced mean and innovation variance. `fitted` and `residuals` retain source-row positions with initial unavailable rows set to null. `forecasts[0]` is the first future observation; `lower` and `upper` are pointwise Gaussian prediction intervals obtained from the integrated impulse weights. They condition on estimated parameters and do not include parameter uncertainty.

`effective_observations`, `iterations`, `log_likelihood`, `aic`, `bic` and `innovation_variance` describe the conditional fit. AIC/BIC count innovation variance as a parameter; compare models on the same transformed estimation sample. Coefficient significance tests are not produced. Failure to converge is reported as failure, without a partial forecast.

[Seasonal ARIMA notation](https://otexts.com/fpp3/seasonal-arima.html)
