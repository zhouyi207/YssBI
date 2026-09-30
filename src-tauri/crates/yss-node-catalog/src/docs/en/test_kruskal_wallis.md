# Kruskal–Wallis test

Compares distributions of independent groups using pooled ranks.

## Inputs and parameters

Add 2–16 numeric inputs under `groups`. Each must be nonempty, finite, and nonmissing. The current node requires equal column lengths; relational series must share a row domain. There are no parameters. Observations must be independent within and between groups.

## Hypotheses and statistic

$H_0$: all population distributions are equal. $H_1$: at least one differs. Let $k$ be group count, $n_i$ group sizes, $N=\sum_i n_i$, $R_i$ pooled rank sums, and $t_j$ tied-group sizes:

$$
H_0^*=\frac{12}{N(N+1)}\sum_{i=1}^{k}\frac{R_i^2}{n_i}-3(N+1),\qquad
C=1-\frac{\sum_j(t_j^3-t_j)}{N^3-N},
$$

$$
H=\frac{H_0^*}{C}\overset{H_0}{\approx}\chi^2_{k-1}.
$$

Ties receive average ranks and $C$ must be positive. A location interpretation requires similar distribution shapes. Chi-square approximation can be inaccurate with small samples.

## Outputs and interpretation

`result` contains the structured result. `statistic` is corrected $H$, `degrees_of_freedom` is `[k−1]`, `p_value` is the upper chi-square tail, and `sample_sizes` contains group sizes. Estimate and standard error are null.

$p<\alpha$ indicates that at least one group differs. The node does not identify the differing pairs or run post-hoc multiple comparisons.
