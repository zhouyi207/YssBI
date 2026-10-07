# VIKOR

Connect aligned Numeric **criteria**, one column per criterion and one row per alternative.
At least two alternatives are required. **cost_criteria** contains one-based column positions
where lower is better. Nulls, non-finite values and unaligned observations are rejected.

Optional **criterion_weights** supplies one finite nonnegative vector in criterion order.
Its sum must be positive; omitted weights are equal. Weights have an independent row domain,
so the output of an entropy or AHP weight computation can be connected.

Each varying criterion becomes a direction-aware best-to-worst loss $l_{ij}$ in [0,1].
Constant criteria contribute zero loss. With normalized weights:

$$
S_i=\sum_j w_j l_{ij},\quad R_i=\max_j w_j l_{ij},\quad
Q_i=v\frac{S_i-S_{\min}}{S_{\max}-S_{\min}}
 +(1-v)\frac{R_i-R_{\min}}{R_{\max}-R_{\min}}.
$$

**majority_weight** is $v$ in [0,1], default 0.5. A zero-range component contributes zero.
Smaller Q is better. **scores** contains observation, Q, S, R and ascending average-tie rank;
**weights** returns normalized weights.

**result** reports two compromise conditions: the Q gap between the top two alternatives
is at least $1/(n-1)$, and the first alternative also minimizes S or R. If the advantage
condition fails, the compromise set contains all alternatives within that strict Q distance
from the best; if only stability fails, it contains the top two. Otherwise it contains
the first alternative. Fully indistinguishable alternatives remain tied in the compromise set.
These are relative decision scores, not statistical significance tests.
