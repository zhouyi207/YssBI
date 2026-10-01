# Dagum Gini decomposition

Connect individual **Values** and observation-level **Group labels**. The columns must have equal lengths and aligned rows. Relational series require one proven row domain and cannot be mixed with independent in-memory series. Labels may be Text, Identifier, Numeric, Categorical, Ordinal, or Binary and retain their original values. Missing labels fail.

Equal-weight empirical Gini, without a small-sample correction, decomposes as

$$G=G_w+G_{nb}+G_t.$$

The components are within-group inequality, net between-group inequality, and transvariation from overlapping distributions. With $p_j=n_j/n$, group mean $\mu_j$, overall mean $\mu$, and $s_j=p_j\mu_j/\mu$,

$$G_w=\sum_jp_js_jG_j,\qquad G_{nb}=\sum_{j>h}\frac{p_jp_h|\mu_j-\mu_h|}{\mu}.$$

Let $A_{jh}=\frac{1}{n_jn_h}\sum_{i\in j,r\in h}|x_i-x_r|$. Pairwise Gini is $A_{jh}/(\mu_j+\mu_h)$, economic distance is $D_{jh}=|\mu_j-\mu_h|/A_{jh}$, and the pair's transvariation contribution is $p_jp_h(A_{jh}-|\mu_j-\mu_h|)/\mu$.

The single `result` includes overall `gini`, `mean`, `observations`, `within`, `between`, `transvariation`, and their `_share` fractions (0–1, not percentages). `groups` contains counts, means, Gini, population/income shares, and within contributions. `pairs` contains pairwise Gini, economic distance, and both between-group contributions. Inspect supports numeric and report views.

Values must be finite, nonnegative, and have a positive overall mean. Missing values and an all-zero population fail. An all-zero subgroup has null Gini and zero within contribution; two all-zero groups have null pairwise Gini. Economic distance is null for zero cross-group difference. Component shares are null when overall Gini is zero. A single group assigns all inequality to the within component. Survey weights, inference, and confidence intervals are not exposed.

For `[1, 2, 3, 4]` grouped as `[A, A, B, B]`, the result is `G=0.25`, `within=0.05`, `between=0.20`, `transvariation=0`. Equal group means can still conceal overlapping distributions.

Method: [Dagum (1997)](https://doi.org/10.1007/BF01205777).
