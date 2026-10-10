# Regression Logit Ordinal

Connect aligned finite observations; missing values are rejected. Unless specified below, Y is numeric and inputs `X₁, X₂, …` accept one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Proportional-odds ordered logit has no separate intercept. Ordinal metadata determines order; plain numeric responses use ascending scores. No implicit text order. Unobserved declared levels are omitted. At least two observed categories and more observations than parameters are required; 500 iterations, tolerance 1e-7. `cut[c|c+1]` are increasing actual thresholds with delta covariance. Positive slopes favor higher categories. Probabilities/predicted labels follow `categories`; numeric residuals and RSS/RMSE are unavailable. Wald inference uses observed information and standard normal reference. Constant predictors/separation/singular information fail.

$$
\operatorname{logit}P(Y\le c\mid x)=\theta_c-x^T\beta,\quad\theta_0<\theta_1<\cdots<\theta_{K-2}.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.miscmodels.ordinal_model.OrderedModel.html)
