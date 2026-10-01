# Phillips–Perron unit-root test

Connect a finite numeric `series` in equally spaced time order, with at least four observations and no missing values. No fixed row ceiling applies. `ts_deterministic=constant` allows `none`, `constant`, or `trend` (intercept plus linear trend). `ts_bandwidth=4` is the nonnegative Newey–West truncation lag, strictly below the regression observation count $N=n-1$; it is not an ADF augmentation order.

$H_0$: the series has a unit root. $H_1$: it is stationary around the selected deterministic terms. Fit levels $y_t$ on $y_{t-1}$ and those terms, then apply the PP tau correction:

$$
Z_\tau=\sqrt{\frac{\gamma_0}{\lambda^2}}\frac{\widehat\rho-1}{SE(\widehat\rho)}-\frac{\lambda^2-\gamma_0}{2\lambda}\frac{N\,SE(\widehat\rho)}{s}.
$$

Here $\gamma_0=\sum e_t^2/N$, $s^2=\sum e_t^2/(N-k)$ and $\lambda^2$ is the Bartlett-weighted Newey–West long-run variance; $k$ counts regression coefficients. Both residual and long-run variances must be positive, and the regression must be full rank with residual degrees of freedom.

`statistic` is PP tau, `p_value` is its MacKinnon response-surface approximation for the chosen deterministic specification, and `p_value_kind=mackinnon_approximation`. The reference is a nonstandard unit-root distribution, not Student t. Small lower-tail p-values reject the unit-root null. `observations`, `effective_observations`, `bandwidth`, `deterministic` and `long_run_variance` record the actual specification; `critical_values` is empty because this node does not compute finite-sample PP critical values. The deterministic specification must follow the data-generating question, not whichever option gives the smallest p-value.

[Phillips–Perron reference](https://arch.readthedocs.io/en/latest/unitroot/generated/arch.unitroot.PhillipsPerron.html)
