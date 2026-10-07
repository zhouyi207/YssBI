# Overall linear regression power

`predictors` defaults to 3: positive number p of predictors excluding the intercept. `effect_f_squared` defaults to 0.15 and is nonnegative overall `R²/(1−R²)`. sample_size is total observations n, at least p+2.

H0 jointly sets all p slopes to zero; the alternative has at least one nonzero slope. The normal homoskedastic model with intercept uses the upper F tail, df `(p,n−p−1)` and noncentrality `nf²`. This plans overall regression under the specified population effect, not individual coefficients, incremental R² or post-selection power.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.
