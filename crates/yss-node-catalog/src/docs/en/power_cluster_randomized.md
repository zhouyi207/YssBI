# Cluster randomized power (continuous outcome)

Two-arm cluster randomized design with continuous normal outcomes and equal cluster sizes. `effect_size` defaults to 0.5: mean difference divided by individual population SD, d. `cluster_size` defaults to 20, positive integer subjects m per cluster; `icc` defaults to 0.05 in [0,1]. sample_size is c clusters per arm, at least 2; total subjects are 2cm.

H0 is zero mean difference, with the selected alternative. Design effect is `DE=1+(m−1)ICC`. The cluster-mean t approximation uses df 2c−2 and noncentrality `d√{cm/(2DE)}`. This equal-cluster-size, exchangeable-correlation plan excludes unequal sizes, binary outcomes, stepped-wedge designs and multilevel covariate adjustment.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.
