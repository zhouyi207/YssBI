# Nonparametric group-test selector

Selects one independent-group nonparametric test through `method`. Each execution runs only that method.

## Inputs and parameters

Add 2–16 nonempty finite numeric series under `groups`, with no missing values. The current node requires equal lengths; relational series must share a row domain. `method` defaults to `mann_whitney`:

| `method`         | Groups and purpose                                                         |
| ---------------- | -------------------------------------------------------------------------- |
| `mann_whitney`   | Exactly 2 groups, two-sided distribution/location comparison               |
| `kruskal_wallis` | 2–16 groups, distribution comparison                                       |
| `mood_median`    | Median comparison; current valid table construction is limited to 2 groups |

There is no `alternative` parameter. Groups are not treated as paired differences. Observations must be independent within and between groups.

## Methods and hypotheses

### Mann–Whitney

$H_0$: both populations have the same distribution. $H_1$: their distributions differ. Rank the pooled data using average ranks for ties. With group-1 rank sum $R_1$:

$$
U_1=R_1-\frac{n_1(n_1+1)}2,\qquad
Z=\frac{U_1-n_1n_2/2}{\sqrt{V(U_1)}}\overset{H_0}{\approx}N(0,1).
$$

$n_i$ are group sizes; $V(U_1)$ includes tie correction. Inference is a two-sided normal approximation with no continuity correction or exact method. A location interpretation requires comparable distribution shapes.

### Kruskal–Wallis

$H_0$: all population distributions are equal. $H_1$: at least one differs. For $k$ groups, total size $N$, and group rank sums $R_i$:

$$
H=\frac{12\sum_i R_i^2/n_i\,/\,[N(N+1)]-3(N+1)}{C}
\overset{H_0}{\approx}\chi^2_{k-1}.
$$

$C=1-\sum_g(t_g^3-t_g)/(N^3-N)$, where $t_g$ are tied-group sizes. All-identical data cannot be tested.

### Mood median

$H_0$: population medians are equal. $H_1$: at least one differs. Build a table of counts strictly above and below the pooled median, excluding values equal to it:

$$
X^2=\sum_{ij}\frac{(O_{ij}-E_{ij})^2}{E_{ij}}
\overset{H_0}{\approx}\chi^2_{k-1}.
$$

$O_{ij}$ are observed counts and $E_{ij}$ expectations under independence. Current table construction does not preserve group margins correctly for more than two groups; multi-group results should not be interpreted.

## Outputs and interpretation

`result` and `report` contain the same report. `method` names the actual method. `statistic` is $U_1$, $H$, or $X^2$, respectively. `degrees_of_freedom` is empty for Mann–Whitney and `[k−1]` otherwise; `sample_sizes` lists input group sizes. Mann–Whitney's `details.u_complement` gives the other U, while Mood's `details.pooled_median` gives the pooled median. Estimate and standard error are null.

For a valid selected method, reject its null when `p_value` is below $\alpha$. The selector does not choose a method automatically or run post-hoc pairwise tests or multiplicity adjustments.
