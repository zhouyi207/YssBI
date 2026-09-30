# Within-group agreement rwg

Supply one group's aligned Numeric **Scale item** columns. Each row is a respondent and each column one item. At least two respondents are needed; up to 64 items are supported. For multiple groups, prepare each group's sample separately.

**Null distribution** defaults to `uniform`. **Response options** defaults to five; scores must be integers from 1 to A and the expected variance is $\sigma_E^2=(A^2-1)/12$. Alternatively choose `specified_variance` and enter a finite positive expected variance on the same scale as the items.

For one item with sample variance $s_j^2$,

$$r_{wg,j}=1-s_j^2/\sigma_E^2.$$

`items` reports each item's raw value, its nonnegative truncated rwg, and observed variance. For J items, use the average observed variance and set $q=\min(1,\bar s^2/\sigma_E^2)$:

$$r_{wg(J)}=\frac{J(1-q)}{J(1-q)+q}.$$

`result` contains the null definition, expected and mean observed variance, item details, aggregate `rwg_j`, and `variance_truncated`. Variance above the null level is truncated for aggregation, producing zero agreement; raw negative item values are retained. This is a descriptive agreement measure, with no p-value or confidence interval. Interpretation depends on a justified null distribution and scale; a universal aggregation cutoff is not applied.

Reference: [multilevel rwg(J)](https://search.r-project.org/CRAN/refmans/multilevel/html/rwg.j.html).
