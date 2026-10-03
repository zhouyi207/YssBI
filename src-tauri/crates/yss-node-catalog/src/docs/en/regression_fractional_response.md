# Regression Fractional Response

Connect aligned finite observations; missing values are rejected. Unless specified below, Y is numeric and inputs `X₁, X₂, …` accept one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Continuous proportions including 0/1; overall mean must be strictly inside (0,1). `response_link=logit` by default; probit/cloglog supported. `constant=true`; 500 iterations, tolerance 1e-7. Bernoulli quasi-likelihood mean equation with HC0 score-sandwich covariance and standard normal inference; this does not assume binomial trial-count variance. A genuine continuous-response likelihood and AIC/BIC are unavailable. This is distinct from Beta and grouped-binomial regression.

$$
\hat\beta=\arg\max_\beta\sum_i[y_i\log\mu_i+(1-y_i)\log(1-\mu_i)],\quad0\le y_i\le1.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.genmod.generalized_linear_model.GLM.html)
