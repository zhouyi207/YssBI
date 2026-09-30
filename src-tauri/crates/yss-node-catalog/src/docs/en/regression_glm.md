# Regression GLM

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts 1–16 numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

`glm_family=poisson` by default. Gaussian supports `gaussian_link=identity` (default) or log; binomial `binomial_link=logit` (default), probit or cloglog; Poisson/Gamma/inverse Gaussian use fixed log. Their variance functions are 1, μ(1−μ), μ, μ², μ³ respectively. Binomial requires 0/1; Poisson nonnegative integer counts and positive mean; Gamma/inverse Gaussian positive response. `constant=true`; 500 iterations, tolerance 1e-7. Gaussian/Gamma/inverse Gaussian estimate Pearson dispersion with n−p degrees; binomial/Poisson use 1. Fisher covariance is dispersion-scaled; Gaussian uses Student t(n−p), others standard normal. `details` includes family/link/dispersion/deviance. No other links or offsets.

$$
g(\mu_i)=x_i^T\beta,\quad\operatorname{Var}(Y_i\mid x_i)=\phi V(\mu_i).
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/glm.html)
