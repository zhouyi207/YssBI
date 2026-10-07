# Balanced one-way ANOVA power

`groups` defaults to 3 and is at least 2. `effect_f` defaults to 0.25: nonnegative Cohen f, the population SD of group means divided by common within-group SD. sample_size is n per group, at least 2; total size is gn.

H0 is equality of all group means; the alternative is at least one difference, using the upper F tail. Assumes independent normal, equal-variance, balanced groups. Degrees of freedom are `(g−1, g(n−1))`, with noncentrality `gnf²`. Power is the noncentral F tail at the central F critical value. Repeated measures, interactions, unequal group sizes and Welch ANOVA are not included.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.

[Reference](https://www.statsmodels.org/stable/generated/statsmodels.stats.power.FTestAnovaPower.html)
