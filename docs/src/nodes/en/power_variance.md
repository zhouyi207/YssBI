# Normal population variance power

`variance_ratio` defaults to 1.5 and is alternative population variance divided by null variance, r>0. sample_size is n independent observations, at least 2.

H0 is r=1, with the selected alternative. For a normal population, `(n−1)S²/σ0²` is chi-square with n−1 df under H0 and r times that variable under the alternative. Power uses the corresponding one- or two-tail central chi-square critical values. This is a one-population variance test, not a two-variance F test, Levene test or precision-interval calculation.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.
