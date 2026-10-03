# Factorial ANOVA

Provide one numeric **Response** and one or more **Factor** series; numeric factor values are category codes. Columns must have equal lengths and pair by current position, including mixed database and in-memory inputs. Missing values are rejected. Each factor needs at least two observed levels; the design must have full rank, include an intercept, with more observations than columns and positive residual variance.

**Factor terms** defaults to `full_factorial`, testing all main effects and interactions of every order; `main_effects` fits additive effects. **Sums of squares** defaults to `type_iii`: I is sequential, ordering main effects by input position before interactions of increasing order; II adjusts for other terms except higher-order relatives; III adjusts for all other terms. Sum-to-zero contrasts make type III main effects comparisons of equally weighted marginal category means. Type I depends on order; types II and III answer different questions in the presence of interactions.

For each term the null is that all its contrast coefficients are zero, versus at least one nonzero coefficient. Main effects describe mean differences averaged over the other factors; interaction describes departure from additivity.

$$SS_T=SSE_R-SSE_A,\qquad F_T=\frac{SS_T/d_T}{SSE_F/\nu}\sim F_{d_T,\nu}.$$

$R$ and $A$ exclude and include the term under the selected rule, $F$ is the final model, $d_T$ is the term's column count, and $\nu=n-p$, with $p$ counting the final intercept and design columns. Inference assumes independent, approximately normal, equal-variance errors. The full interaction model requires every factor combination to be observed.

The single **Result** has a `table` of sums of squares, degrees of freedom, mean squares, F, p-values and partial $\eta^2$, plus `error`, total variation, total degrees of freedom, $R^2$, options and original labels. `factor1`, `factor2`, etc. follow input order; interaction names join them with colons, e.g. `factor1:factor2:factor3`. Type II/III term sums of squares generally do not sum to model variation.

See the [car Anova documentation](https://cran.r-project.org/web/packages/car/car.pdf) for sums-of-squares definitions.
