# Regression Beta

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Continuous response strictly between 0 and 1; endpoints are rejected, not shifted. Jointly estimate mean coefficients and constant precision by likelihood. `constant=true`; 500 iterations, tolerance 1e-7. Final coefficient `precision` uses positive original scale with delta covariance. Fitted values are conditional means; inference uses observed information and standard normal reference. No precision predictors or zero/one-inflated Beta variant.

$$
Y_i\sim\operatorname{Beta}(\mu_i\phi,(1-\mu_i)\phi),\quad\operatorname{logit}\mu_i=x_i^T\beta,\quad\phi>0.
$$

Positive dispersion/scale rows retain delta standard errors and log-scale transformed 95% intervals. Their zero-boundary statistic/p-value is null.

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.othermod.betareg.BetaModel.html)
