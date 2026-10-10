# Holt–Winters forecast

Choose additive or multiplicative seasonality, with an optional additive or damped trend. Multiplicative seasonality requires strictly positive observations and positive evolving seasonal baselines.

## Input and settings

Connect a finite numeric `series`, ordered and equally spaced, without missing values. No fixed row limit applies. `ts_alpha=0.2` is the level coefficient in [0,1]. `ts_optimize=true` estimates active smoothing coefficients by conditional squared-error minimization; switch it off to use the supplied coefficients. With optimization, supplied values initialize the search, and endpoints are moved just inside the admissible region. `ts_trend=true`, `ts_damped=false`; damping requires an enabled trend. `ts_seasonality=additive` offers `additive/multiplicative`. `ts_period=12` is an integer at least 2, used only with seasonality. `ts_beta=0.1`, `ts_gamma=0.1`, `ts_phi=0.98` are trend, seasonal and damping coefficients in [0,1]; seasonal smoothing additionally requires alpha + gamma ≤ 1. Inactive component coefficients do not enter the recursion.

`ts_horizon=10` is positive. `max_iterations=500` is positive and `tolerance=0.000001` lies in $[10^{-12},0.01]$. Without seasonality, the first observation initializes the level; an enabled trend uses the first difference. With seasonality, at least two complete cycles are needed: their mean difference divided by the period initializes the per-step trend, the first-cycle mean adjusted to its last time point initializes level, and deviations/ratios initialize seasonal states. These initial states remain fixed during coefficient estimation. At least two observations are required; with optimization the fitted sample must exceed the number of estimated smoothing coefficients.

## Recursion and outputs

$$
v_t=\ell_{t-1}+\phi b_{t-1},\quad \ell_t=\alpha(y_t-s_{t-m})+(1-\alpha)v_t,\quad b_t=\beta(\ell_t-\ell_{t-1})+(1-\beta)\phi b_{t-1},\quad s_t=\gamma(y_t-v_t)+(1-\gamma)s_{t-m}.
$$

$\ell$ is level, $b$ trend and $s$ the seasonal state. This is the additive-seasonal recursion; omit the seasonal term when disabled, set trend to zero when disabled, and use phi = 1 without damping. The innovations trend coefficient is alpha × beta. Forecasts extend the final level by the sum of damped trends and reuse the final seasonal cycle. With multiplicative seasonality, replace subtraction/addition of the seasonal component by division/multiplication, and update seasonality from y / v.

`parameters` contains active smoothing coefficients. `fitted` and `residuals` retain source-row positions, with the first observation or first seasonal cycle set to null. `forecasts[0]` is one period after the sample. `innovation_variance` is mean squared error over fitted rows. `log_likelihood`, `aic` and `bic` use conditional Gaussian additive errors and fixed initialization; the parameter count includes estimated smoothing coefficients and variance. If the fit is exact, these three likelihood quantities are unavailable. Do not compare criteria across different initialization samples. `iterations` reports optimization work. Prediction intervals and coefficient significance tests are not supplied. Initialization uses sample data, so fitted errors are not a rolling-origin backtest.

[Exponential smoothing models](https://www.statsmodels.org/stable/generated/statsmodels.tsa.exponential_smoothing.ets.ETSModel.html)
