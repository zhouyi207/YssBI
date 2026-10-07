# Multivariate ANOVA (MANOVA)

Jointly compare factor effects across multiple responses. Provide at least two numeric **Y₁, Y₂, …** series and one or more **Factor** series. Columns must have equal lengths and pair by current position; database and in-memory inputs may be mixed. Missing values are rejected. Each factor needs at least two observed categories. The intercept-inclusive design needs full rank and positive residual degrees of freedom. Residual SSCP must be positive definite; collinear responses cannot be jointly tested.

**Factor terms** defaults to `full_factorial`, with `main_effects` available. **Sums of squares** defaults to `type_iii`: I is sequential, ordering main effects by input position before increasing-order interactions; II adjusts for other terms except higher-order relatives; III uses sum contrasts and adjusts for all other terms.

For each term the null sets all associated contrast coefficients to zero across all responses; the alternative has at least one nonzero coefficient. Let $E$ be the final residual sum-of-squares and cross-products matrix, $H=E_R-E_A$ the term's hypothesis SSCP, and $\lambda_i$ the eigenvalues of $E^{-1}H$:

$$\Lambda=\prod_i(1+\lambda_i)^{-1},\qquad V=\sum_i\frac{\lambda_i}{1+\lambda_i},\qquad U=\sum_i\lambda_i,\qquad \Theta=\max_i\lambda_i.$$

These are Wilks lambda, Pillai trace, Hotelling–Lawley trace and Roy greatest root. Standard F transforms/approximations assume independent observations, approximately multivariate normal errors and common group covariance matrices. Roy's F approximation is an upper bound and can be optimistic for multidimensional hypotheses.

The single **Result** includes each term's `hypothesis_df`, `hypothesis_sscp` and four `tests` in `table`, plus `error_df`, `error_sscp`, response count and original labels. SSCP matrices are row-major nested arrays in response input order. Tests include `statistic`, F, numerator/denominator degrees of freedom and p-value. In small samples without valid F degrees of freedom, inference fields are null while the statistic is retained. A small p-value supports differences in response mean vectors without identifying individual responses or category pairs.

See [statsmodels multivariate tests](https://www.statsmodels.org/stable/_modules/statsmodels/multivariate/multivariate_ols.html) for definitions and F transforms.
