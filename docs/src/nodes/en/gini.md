# Gini coefficient

Connect individual observations to **Values**. Observations have equal weight. The empirical Gini uses no small-sample correction:

$$G=\frac{\sum_{i=1}^{n}\sum_{j=1}^{n}|x_i-x_j|}{2n^2\bar x}.$$

Here $n$ is the observation count and $\bar x$ the mean. Values must be nonempty, finite, nonnegative, and have a positive mean. Zeros are allowed; missing, negative, and all-zero inputs fail. One positive observation gives 0. The upper bound is $(n-1)/n$; no $n/(n-1)$ multiplier is applied. Larger values indicate greater inequality.

The single `result` contains `gini`, `mean`, and `observations`. For example, `[1, 2, 3, 4]` gives `0.25`. Inspect supports both numeric and report views.

Supply individual observations on the same scale. Group means omit within-group inequality. Frequency/survey weights and confidence intervals are not exposed. Use Dagum with an observation-level grouping column for subgroup decomposition.

Convention: [ineq Gini and its correction option](https://stat.ethz.ch/CRAN/web/packages/ineq/refman/ineq.html).
