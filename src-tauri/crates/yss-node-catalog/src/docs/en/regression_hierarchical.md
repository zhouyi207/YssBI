# Regression Hierarchical

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts 1–16 numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Cumulatively enter blocks in predictor-port order. `block_sizes` contains positive integers summing to predictor count; empty enters one predictor per block. `constant=true`. This is block-entry OLS, not multilevel/random effects. `stages` retains each model and original one-based predictor positions. Later stages retain ΔR² and a joint test of q added coefficients under H0: all zero, with F(q,n−p_current) reference. Require full rank and positive residual degrees; perfect fits leave change F/p null. Coefficient t inference is conditional on specified blocks.

$$
F_{\rm change}=\frac{(RSS_{\rm previous}-RSS_{\rm current})/q}{RSS_{\rm current}/(n-p_{\rm current})}.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.RegressionResults.compare_f_test.html)
