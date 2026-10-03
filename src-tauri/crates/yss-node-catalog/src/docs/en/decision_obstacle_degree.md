# Obstacle degree

Connect aligned Numeric **criteria** and optionally a separate **criterion_weights** vector.
Weights follow criterion order, must be nonnegative and finite, and are normalized to sum
to one; omitted weights are equal. Nulls and non-finite observations are rejected.

**rescale** defaults to true: use min–max scaling, with constant columns mapped to zero.
If false, input values must already lie in [0,1]. **cost_criteria** lists one-based
positions to reverse into benefit utilities; constant min–max columns stay zero.

For oriented utilities $z_{ij}$, the deviation and obstacle share are

$$
d_{ij}=1-z_{ij},\qquad O_{ij}=100\frac{w_jd_{ij}}{\sum_k w_kd_{ik}}.
$$

**scores** is a pageable long table with one row per observation and criterion: observation,
criterion, deviation, weighted deviation and obstacle percent. Shares total 100 per
observation when weighted deviation is positive. If every weighted deviation is zero,
shares are undefined and remain null; **result.undefined_rows** counts such observations.
**weights** returns the normalized vector. There is no fixed row-count limit.

This is a single-level criterion attribution. For a hierarchy, supply combined global
weights after constructing local and parent weights explicitly. A large share identifies
a large weighted shortfall, not evidence of a causal barrier.
