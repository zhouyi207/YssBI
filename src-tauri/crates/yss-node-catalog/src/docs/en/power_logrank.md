# Log-rank power (Schoenfeld approximation)

`hazard_ratio` defaults to a positive 0.7, `event_fraction` to 0.5 in (0,1], and `allocation` to treatment fraction q=0.5 in (0,1). sample_size is the combined total n, at least 1. Allocation is a continuous planning fraction; arm sizes are not separately rounded.

H0 is HR=1, with the selected alternative. Schoenfeld normal shift is `log(HR)√{n×event_fraction×q(1−q)}` and expected_events is n×event_fraction. Assumes proportional hazards, independent observations and noninformative censoring. Accrual, follow-up, crossover and distinct arm-specific event processes are not simulated. Event-allocation approximation may deteriorate with very unequal allocation or large effects.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.

[Reference](https://arxiv.org/abs/2407.03420)
