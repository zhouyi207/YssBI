# Regression Logit Firth

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Binary response is 0/1 or false/true with both outcomes. `constant=true`; 500 iterations, tolerance 1e-7. Jeffreys-penalized logistic likelihood can give finite estimates under separation with a full-rank design. `details.penalized_log_likelihood` uses original predictor units. Inference uses unpenalized Fisher information and normal Wald approximation, not profile penalized likelihood. Ordinary log likelihood is retained; unpenalized AIC/BIC/LR tests are not supplied.

$$
\ell^*(\beta)=\ell(\beta)+\tfrac12\log|X^TWX|,\quad W_{ii}=\pi_i(1-\pi_i).
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://search.r-project.org/CRAN/refmans/logistf/html/logistf.html)
