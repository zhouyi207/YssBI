# Regression Zero Inflated Negative Binomial

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts 1–16 numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Nonnegative integer counts with positive mean; f is NB2. Count terms use `predictors`; up to 16 `inflation_predictors` configure the zero-generation logit, which always has an intercept and defaults to intercept only. `constant=true` controls the count intercept. At most 64 combined parameters; 500 iterations, tolerance 1e-7. Order: count terms, `inflation.*`, then `alpha`. Fitted values are mixture means (1−π)μ. Inference uses the complete observed Hessian including cross-component covariance and normal Wald reference. The local optimizer does not guarantee a global mixture maximum. No offset/exposure/hurdle variant.

$$
P(Y=0)=\pi+(1-\pi)f(0;\mu),\quad P(Y=y>0)=(1-\pi)f(y;\mu),\quad\log\mu=x^T\beta,\quad\operatorname{logit}\pi=z^T\gamma.
$$

Positive dispersion/scale rows retain delta standard errors and log-scale transformed 95% intervals. Their zero-boundary statistic/p-value is null.

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.discrete.count_model.ZeroInflatedNegativeBinomialP.html)
