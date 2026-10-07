# Paired t-test power

`effect_size` defaults to 0.5: true mean paired difference divided by the population SD of differences, not the SD of individual measurements. sample_size is n complete pairs, at least 2; total_observations counts 2n measurements.

H0 is zero mean difference, with the selected alternative. Pairs are independent and differences normally distributed. The noncentral t has df n−1 and noncentrality `d√n`. Within-pair correlation is already represented by the SD of differences and is not applied again.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.

[Reference](https://www.stat.ethz.ch/R-manual/R-devel/library/stats/html/power.t.test.html)
