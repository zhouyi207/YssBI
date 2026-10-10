# PSM price sensitivity

Connect aligned price responses: **too_cheap**, **cheap**, **expensive**,
**too_expensive**. Prices must be finite and nonnegative, in this nondecreasing
order within every row. Invalid/missing rows raise an error.

**curves** contains every distinct observed price, the four curves, and the
complements not_cheap and not_expensive. Expensive curves use F(x) = P(X ≤ x);
cheap curves use 1 − F(x). Adjacent observed points are linearly joined.

**range_definition** selects original Van Westendorp bounds (default):
too_cheap ∩ not_cheap and not_expensive ∩ too_expensive;
or narrower bounds: too_cheap ∩ expensive and cheap ∩ too_expensive.
**result** also returns indifference (cheap ∩ expensive) and the conventional
“optimal” point (too_cheap ∩ too_expensive).
Each intersection preserves its lower/upper endpoints if curves coincide;
no intersection within the observed range returns null. There is no extrapolation.

These are perception-based intersections, not revenue-maximizing prices.
Reference: [method definitions](https://max-alletsee.github.io/pricesensitivitymeter/reference/psm_analysis.html).
