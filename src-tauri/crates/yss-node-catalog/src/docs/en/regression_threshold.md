# Regression Threshold

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Add aligned `threshold_variable` q. Single unknown threshold, separate coefficients on both sides; `constant=true`. `trimming=0.15` (0.05–0.45); each regime also needs p+2 rows. Distinct observed candidates satisfying sizes are searched; `max_candidates=100` (1–200) evenly samples larger sorted candidate sets. Minimum RSS wins, ties choose first. Terms `below.*`/`above.*` mean q≤c/q>c. Details retain cutoff, counts and search size. Pooled-variance t(n−2p) inference treats the selected threshold as fixed; no threshold-significance/search-adjusted test. No panel or multiple thresholds.

$$
(\hat c,\hat\beta_1,\hat\beta_2)=\arg\min_{c,\beta_1,\beta_2}\sum_i[y_i-\mathbf1(q_i\le c)x_i^T\beta_1-\mathbf1(q_i>c)x_i^T\beta_2]^2.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.ssc.wisc.edu/~bhansen/papers/ecnmt_00.pdf)
