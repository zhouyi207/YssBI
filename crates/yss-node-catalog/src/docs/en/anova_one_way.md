# One-way ANOVA

Compare population means across independent groups. The null is $H_0:\mu_1=\cdots=\mu_k$; the alternative is that at least one mean differs.

**Y** is a numeric series; **Factor** is one grouping series with at least two observed categories. Numeric codes are treated as categories. Columns must align row by row; database and in-memory series may be mixed and pair by current position. Missing values are rejected. There must be more observations than groups and positive within-group residual variance.

$$F=\frac{SS_{\mathrm{between}}/(k-1)}{SS_{\mathrm{within}}/(n-k)}\sim F_{k-1,n-k}.$$

Here $n$ is the observation count and $k$ the group count; between-group and within-group sums of squares measure differences in group means and within-group variation. Classical inference assumes independent observations, approximately normal errors and equal group variances. This node computes classical ANOVA without a Welch variant.

The single **Result** contains a `table` with sums of squares, degrees of freedom, mean squares, F, p-values and partial $\eta^2=SS_{\mathrm{effect}}/(SS_{\mathrm{effect}}+SS_{\mathrm{error}})$, plus `error`, total variation, total degrees of freedom, $R^2$ and original factor labels. A small p-value supports unequal group means without identifying which pairs differ.
