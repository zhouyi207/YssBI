# Cox power (Schoenfeld approximation)

`hazard_ratio` defaults to a positive 0.7; `event_fraction` defaults to 0.5 in (0,1], the expected fraction of enrolled subjects with observed events. `predictor_variance` defaults to 0.25, the tested predictor's positive population variance. sample_size is total enrolled subjects n, at least 1.

H0 is HR=1, with the selected alternative. With expected events D=n×event_fraction and predictor variance v, the Schoenfeld normal approximation has shift `log(HR)√(Dv)`. expected_events reports D, not a guaranteed event count. Assumes proportional hazards, noninformative censoring and credible planning effects/event fraction. Other covariate adjustments, time-varying HR, competing risks and accrual/follow-up simulation are not included.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.

[Reference](https://arxiv.org/abs/2407.03420)
