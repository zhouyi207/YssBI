# Theil index

Computes **Theil T** with natural logarithms. Select **Data form** in Details.

- **Individual** (default): connect one numeric series of individual values. Each observation has equal weight; remove the Weights input if switching from grouped data.
- **Grouped**: connect a series of **group means**, then add one **Weights (group population)** input in Details and connect the corresponding group populations or population shares. Weights are normalized automatically; counts and proportional shares give the same result. Do not supply group totals as the values.

For values $x_i$, let $p_i=w_i/\sum_j w_j$ (or $p_i=1/n$ for individuals), $\mu=\sum_i p_i x_i$, and $q_i=p_i x_i/\mu$. The result is

$$T=\sum_{i:q_i>0} q_i\ln(q_i/p_i).$$

Values and weights must be finite and nonnegative. Zero values contribute zero; zero-weight groups are ignored. The total weight and weighted mean must be positive. Empty inputs, missing values, negative values, or mismatched lengths fail explicitly. Relational series must share a proven row domain; in-memory series align by position. Filter the common sample upstream instead of dropping values from each series independently.

The single **Result** output contains structured data with `theil_t`, `form`, and `observations` (the number of supplied individuals or groups, including zero-weight groups). Equal positive values produce zero up to floating-point roundoff; larger values indicate more inequality. The index is not restricted to 0–1.

Group means measure **between-group inequality**. Without within-group information, this does not recover overall inequality or a within/between decomposition. For example, group means `[1, 3]` and weights `[3, 1]` give the same index as individual values `[1, 1, 1, 3]`, approximately `0.1438410362`.

Formula and decomposition: [World Bank, Policy Research Working Paper 3919](https://documents1.worldbank.org/curated/en/122811468049747658/pdf/wps3919.pdf).
