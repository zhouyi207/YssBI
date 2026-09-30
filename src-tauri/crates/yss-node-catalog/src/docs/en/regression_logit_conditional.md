# Regression Logit Conditional

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts 1–16 numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Binary response plus aligned category `groups`, at most 64 strata. Conditional binary logit conditions on each case count m_g and estimates no intercept. All-zero/all-one strata are dropped with counts in `details`; `observations` counts informative rows. Predictors require within-stratum variation and full rank. Default 500 iterations, tolerance 1e-7. Normal Wald inference uses observed conditional information. Absolute fitted probabilities, numeric residuals, RSS/RMSE are unavailable because intercepts are conditioned out. This is matched/stratified binary logit, not multinomial choice sets.

$$
L(\beta)=\prod_g\frac{\exp(\sum_{i\in g}y_ix_i^T\beta)}{\sum_{A\subseteq g,\ |A|=m_g}\exp(\sum_{i\in A}x_i^T\beta)},\quad m_g=\sum_{i\in g}y_i.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.discrete.conditional_models.ConditionalLogit.html)
