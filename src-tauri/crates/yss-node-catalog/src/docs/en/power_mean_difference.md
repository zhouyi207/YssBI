# Mean difference power (t test)

`effect_size` defaults to 0.5 and is the signed true mean difference divided by population SD, d. `design=independent` (default) compares two independent normal populations with equal variances and sample sizes. sample_size is n per group, at least 2; total size is 2n. Noncentral t degrees of freedom are 2n−2 and noncentrality is `d√(n/2)`.

`design=one_sample` tests one normal population mean; d is true minus null mean divided by population SD. sample_size is total observations n, at least 2, with df n−1 and noncentrality `d√n`. H0 is zero standardized mean difference, with the selected alternative. Power is the noncentral t rejection probability beyond central t critical values. The independent design does not include Welch unequal-variance tests or unequal group sizes.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.

[Reference](https://www.stat.ethz.ch/R-manual/R-devel/library/stats/html/power.t.test.html)
