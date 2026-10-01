# Analysis of covariance (ANCOVA)

Compare factor effects after adjusting for continuous covariates. Provide one numeric **Response**, one or more **Factor** series and one or more numeric **Covariate** series. Factors need at least two observed categories; numeric codes remain categories. All columns must align and relational series must share a row domain. Missing values are rejected. The intercept-inclusive design must have full rank, more observations than columns and positive residual variance.

**Factor terms** defaults to `full_factorial`, including all factor interactions; `main_effects` includes only additive factor effects. Covariates are centered at their observed means and always enter as additive linear terms with parallel slopes across groups. Factor-by-covariate interactions are excluded.

**Sums of squares** defaults to `type_iii`. Type I enters covariates in input order before main effects and increasing-order factor interactions. Type II adjusts for other terms except higher-order relatives. Type III uses sum contrasts and adjusts for every other term, comparing equally weighted factor marginal means at the covariate means.

For each factor or covariate term, $H_0$ sets all associated contrast coefficients to zero, versus at least one nonzero coefficient:

$$SS_T=SSE_R-SSE_A,\qquad F_T=\frac{SS_T/d_T}{SSE_F/(n-p)}\sim F_{d_T,n-p}.$$

$R$ and $A$ exclude and include the term under the selected rule, $F$ is the final model, $d_T$ counts the term's columns and $p$ includes the intercept. Inference assumes independent, approximately normal, equal-variance errors, linear covariate relationships and common slopes. Full factor interactions require complete factor combinations. A parallel-slopes model is inappropriate when slopes differ by group.

The single **Result** reports a `table` of sums of squares, degrees of freedom, mean squares, F, p-values and partial $\eta^2$, plus `error`, total variation, $R^2$, `covariate_means` and original labels. `factor1`, `covariate1`, etc. follow their respective input order. A small factor p-value supports adjusted group differences; adjusted means and post-hoc comparisons are not computed.
