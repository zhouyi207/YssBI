# Grey prediction GM(1,1)

Connect an ordered, equally spaced numeric `series` containing at least four strictly positive, finite observations. Missing values are rejected. `ts_horizon=10` is a positive integer; no fixed input row ceiling applies.

The accumulated series is $X_k=\sum_{i=0}^k y_i$. Estimate $a,b$ by least squares in

$$
y_k=-a\frac{X_{k-1}+X_k}{2}+b,\qquad k=1,\ldots,n-1.
$$

The time response is $\widehat X_k=(y_0-b/a)e^{-ak}+b/a$, and restored predictions are $\widehat y_k=\widehat X_k-\widehat X_{k-1}$. The continuous limit at $a=0$ is used. A full-rank design is required.

`parameters` contains `development_a` and `input_b`. `fitted` and `residuals` retain source-row alignment; their first row is null because the first value initializes the model. `forecasts[0]` predicts the first period after the sample. `innovation_variance` is the restored-response mean squared error, not a stochastic grey-model variance estimate. Likelihood, AIC/BIC and prediction intervals are unavailable; no hypothesis test is implied.

GM(1,1) imposes an approximately exponential pattern. Positivity alone does not establish suitability: inspect restored residuals, level ratios and held-out forecasts. The node does not automatically shift nonpositive data or search preprocessing variants.
