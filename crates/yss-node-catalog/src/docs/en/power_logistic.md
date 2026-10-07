# Logistic power (binary predictor)

Supports Logistic Wald planning with one binary predictor, two independent equal-size groups and no other covariates. `baseline_probability` defaults to 0.2 in (0,1); `odds_ratio` defaults to a positive 1.5. sample_size is n per group, at least 1.

H0 is OR=1, with the selected alternative. `logit(p1)=logit(p0)+log(OR)` determines the other group's probability. Approximate variance of log(OR) is `[1/{p0(1−p0)}+1/{p1(1−p1)}]/n`. Divide log(OR) by this SE for the normal mean shift. Small samples, rare outcomes and separation can impair the approximation. Multivariable adjustment, continuous predictors and prediction-model-development sample sizes are not included.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.

[Reference](https://www.ncss.com/software/pass/regression-in-pass/)
