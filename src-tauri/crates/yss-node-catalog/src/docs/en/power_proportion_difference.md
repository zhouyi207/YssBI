# Two-proportion power (normal approximation)

`proportion1` defaults to 0.6 and `proportion2` to 0.4; both lie in [0,1] and cannot both be zero or both one. Groups are independent and equally sized. sample_size is n per group, at least 1; total size is 2n. H0 is p1−p2=0, with the selected alternative.

Let p̄=(p1+p2)/2. Null planning SE is `√[2p̄(1−p̄)/n]`; alternative SE is `√[{p1(1−p1)+p2(1−p2)}/n]`. The former defines normal critical boundaries; the true difference and latter determine rejection probability. This is an uncorrected normal approximation, not an exact binomial/Fisher test. Small samples, rare events and boundary proportions need care.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.

[Reference](https://www.stat.ethz.ch/R-manual/R-devel/library/stats/html/power.prop.test.html)
