# One-proportion power (normal approximation)

`null_proportion` defaults to 0.5 and must lie in (0,1); `proportion` defaults to 0.6 and lies in [0,1]. sample_size is total observations n, at least 1. H0 is p=p0, with the selected alternative.

Uses a normal approximation without continuity correction. Null SE is `√[p0(1−p0)/n]`; the alternative estimate has mean p and SE `√[p(1−p)/n]`. The rejection region uses null SE; power uses the alternative distribution. Boundary alternatives p=0 or 1 use a point mass. Approximation can be poor for small samples or rare events; this is not an exact binomial test.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.
