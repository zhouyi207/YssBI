# Regression PLS

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Fit univariate PLS1 with covariance directions and regression deflation. `components=1`, a positive integer no greater than predictor count, centered rank or n−1. Response/predictors are centered; `standardize=true` scales predictors. An intercept is reconstructed in original units. W, P and q are predictor weights/loadings and response loadings, retained in component order in `details`. A component without remaining response covariance fails. No cross-validation or coefficient inference.

$$
\hat\beta=W(P^TW)^{-1}q.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://scikit-learn.org/stable/modules/generated/sklearn.cross_decomposition.PLSRegression.html)
