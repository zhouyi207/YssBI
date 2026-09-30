# Mood median test

Compares independent-group medians through position relative to the pooled median.

## Inputs and parameters

Add 2–16 nonempty finite numeric series under `groups`. The current node requires equal lengths; relational series must share a row domain. Missing values are rejected. There are no parameters.

Compute the pooled median $m$, then count observations strictly above and strictly below $m$ in each group. Observations exactly equal to $m$ are excluded from the test table.

## Hypotheses and statistic

$H_0$: all population medians are equal. $H_1$: at least one differs. For the table crossing the two position categories with group:

$$
E_{ij}=\frac{O_{i+}O_{+j}}N,\qquad
X^2=\sum_{ij}\frac{(O_{ij}-E_{ij})^2}{E_{ij}}
\overset{H_0}{\approx}\chi^2_{k-1}.
$$

Here $k$ is group count and $N$ is the total after excluding median ties. Independent samples and positive table margins are required. Many median ties reduce the information used.

## Outputs and current limitation

`result` contains the structured result. `statistic` is chi-square, `degrees_of_freedom` is `[k−1]`, `p_value` is the upper tail, and `details.pooled_median` is $m$. `sample_sizes` retains group sizes before excluding median ties. Estimate and standard error are null.

The current table order gives an equivalent independence test for two groups. With more than two groups it does not preserve group margins correctly, so results should not be used for multi-group median inference. For valid two-group results, $p<\alpha$ supports different medians.
