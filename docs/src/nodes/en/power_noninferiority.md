# Mean noninferiority power (known variance)

Two independent equal-size normal groups with known common population SD. `difference` defaults to 0: true mean difference divided by that SD, δ. `margin` defaults to 0.3: positive noninferiority margin M in the same units. sample_size is n per group, at least 1.

`higher_is_better=true` (default) uses H0: δ≤−M versus H1: δ>−M. False reverses δ, equivalent to H0: original δ≥M. One-sided normal power is `Φ{(favorable-direction δ+M)√(n/2)−z(1−α)}`. This known-variance mean plan is not a proportion, hazard-ratio or unknown-variance t design. Sample-size solving requires the planned difference inside the alternative unless the minimum sample already meets target.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.
