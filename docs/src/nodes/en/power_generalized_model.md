# GLM power (Poisson rate ratio)

Implements Poisson rate-ratio Wald planning for two independent, equally sized groups with equal exposure. `baseline_rate` defaults to 1 (positive rate λ0 per exposure unit), `rate_ratio` to 1.5 (positive RR), and `exposure` to 1 (positive exposure e per subject). sample_size is n subjects per group, at least 1.

H0 is RR=1, with the selected alternative. With λ1=RR×λ0, planning variance of log(RR) is `(1/λ0+1/λ1)/(ne)`. Its standardized shift is log(RR) divided by this SE; power uses normal tails. Assumes Poisson mean/variance behavior, without overdispersion, zero inflation or additional predictors. This node does not claim arbitrary-GLM power.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.
