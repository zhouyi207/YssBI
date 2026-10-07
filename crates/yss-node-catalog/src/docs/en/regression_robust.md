# Regression Robust

Connect aligned finite observations; missing values are rejected. Unless specified below, Y is numeric and inputs `X₁, X₂, …` accept one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

`robust_loss` selects Huber (default) or Tukey bisquare. Positive `tuning` defaults to 1.345; Tukey commonly uses 4.685. Scale s is the residual MAD about zero divided by 0.67448975, updated during IRLS. `constant=true`; maximum iterations 500, tolerance 1e-7. This is an M-estimator, distinct from choosing HC/cluster standard errors for OLS. Inference uses H1 covariance and a standard normal reference. No robust-model likelihood or AIC/BIC is reported.

$$
\hat\beta=\arg\min_\beta\sum_i\rho((y_i-x_i^T\beta)/s).
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.robust.robust_linear_model.RLM.html)
