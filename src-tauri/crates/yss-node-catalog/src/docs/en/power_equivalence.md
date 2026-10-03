# Mean equivalence power (known variance)

Two independent equal-size normal groups with known common population SD. `difference` defaults to 0: true mean difference divided by that SD, δ. `margin` defaults to 0.3: positive symmetric equivalence margin M in the same units. sample_size is n per group, at least 1.

H0 is δ≤−M or δ≥M; H1 is −M<δ<M. TOST requires rejection by both one-sided tests, each at level α. With s=√(2/n), z=z(1−α), power is `max[0, Φ{(M−δ)/s−z}−Φ{(−M−δ)/s+z}]`. Equivalence is distinct from a nonsignificant difference; for α<0.5 its corresponding confidence level is 1−2α. Solving sample size requires δ strictly inside the equivalence interval unless the minimum sample already meets target. Unknown-variance t TOST, asymmetric margins and log-scale bioequivalence are not included.

No data input is required; parameters describe a planned study. `solve_for=power` (default) evaluates `sample_size` (default 100). `sample_size` mode finds the minimum integer achieving `target_power` (default 0.8, strictly between 0 and 1). The other mode's sample-size/target parameter is ignored. `alpha` defaults to 0.05 and lies strictly between 0 and 1.

Where alternative is available, it defaults to `two_sided`, with `greater` and `less` also supported. Two-sided power includes both rejection tails, including the direction opposite the planned effect. Power is rejection probability under a specified alternative, not a data-derived p value or probability that H0 is false.

**result** reports model, method, alpha, alternative, sample_size, sample_unit, total_observations, power, type_ii_error (1−power), target_power when solving, and expected_events for survival designs. Power is recomputed at the returned integer sample size. Unreachable targets, invalid designs and numerical nonconvergence fail. Counts must be exactly representable and numerically computable; no observation dataset is read or truncated. Planning effects should come from study hypotheses or credible external information, not observed-significance power calculations.

[Reference](https://www.statsmodels.org/stable/generated/statsmodels.stats.weightstats.ztost.html)
