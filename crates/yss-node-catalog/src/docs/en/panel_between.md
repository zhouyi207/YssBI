# Between estimator

Connect numeric response, one or more predictors, entity IDs and time. Inputs must be aligned, finite and complete, with unique entity-time keys.

**Parameters:** constant defaults to true; effects defaults to entity, with time as the alternative. Covariance is nonrobust.

entity averages observations by entity and estimates $\bar y_i=\alpha+\bar x_i'\beta+\bar\varepsilon_i$. time instead uses cross-entity means by period. Group means receive equal weight, regardless of their observation counts. Disabling constant fits an equation without an intercept.

The only output, **model**, connects to Panel Summary. Each estimation row is a group mean; sourceRows retains all original rows in that group. These are not individual-observation fitted values. Two-sided coefficient t tests use the null $H_0:\beta_j=0$ and residual degrees of freedom equal to group count minus design rank.

Sufficient groups and between-group variation are required. This estimator uses between-group information and does not identify within-entity change effects.
