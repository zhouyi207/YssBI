# Correlation power (Fisher z)

`null_correlation` defaults to 0 and `correlation` to 0.3; both lie strictly in (−1,1). sample_size is n paired observations, at least 4, each containing two variables. H0 is ρ=ρ0, with the selected alternative.

For independent bivariate normal observations, the standardized Fisher-transform shift is `(atanh(ρ)−atanh(ρ0))√(n−3)`. Power uses approximate standard normal critical values and tails. No small-sample bias correction is applied. This is not the exact correlation distribution or a comparison of dependent correlations.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.
