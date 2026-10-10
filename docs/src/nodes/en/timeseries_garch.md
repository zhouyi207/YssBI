# GARCH model

Connect an equally spaced, ordered, finite numeric `series`, usually returns or an already stationary residual series. Missing values are rejected and no row limit is imposed. `constant=true` estimates a constant mean; false fixes the mean to zero. The node does not fit an ARMA conditional mean or automatically difference prices.

`ts_p=1` is a positive shock-lag order. `ts_q=1` is a nonnegative variance-lag order. Asymmetric models use the same shock order for asymmetric terms. `ts_horizon=10` is positive; `max_iterations=500` is positive and `tolerance=0.000001` lies in $[10^{-12},0.01]$. The sample must exceed the number of fitted parameters and each lag order, and have positive variance.

$$
h_t=\omega+\sum_{i=1}^p\alpha_i\varepsilon_{t-i}^2+\sum_{j=1}^q\beta_j h_{t-j}.
$$

$h_t$ is conditional variance, $\varepsilon_t=y_t-\mu$ the residual. Omega is positive and shock/variance coefficients are nonnegative; sum(alpha) + sum(beta) is below one. This specifies a covariance-stationary model under symmetric Gaussian innovations.

Gaussian conditional maximum likelihood uses all input observations, with the centered sample variance (divisor n) as the fixed presample variance. Presample squared shocks equal that variance, negative-shock squares use half of it, and centered standardized EGARCH presample shocks are zero. Forecast variances use the analytic recursion: future squared shocks are replaced by their conditional expected variances.

`parameters` reports the mean when enabled, omega, and lag coefficients on the input scale. `residuals`, `standardized_residuals` and `conditional_variances` align with every input row. `forecast_variances[0]` is next-period variance, not standard deviation or a price forecast. `log_likelihood`, `aic`, `bic`, `observations` and `iterations` describe the fit. EGARCH also records `simulations` and `seed`. No coefficient standard errors, Wald tests or volatility confidence intervals are supplied. Nonconvergence or a nonfinite recursion fails the calculation.

[Conditional volatility models](https://arch.readthedocs.io/en/latest/univariate/generated/arch.univariate.GARCH.html)
