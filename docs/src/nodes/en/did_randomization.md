# DID Fake-group Randomization

Connect aligned finite Y, optional X₁, X₂, …, entity, time, treat and post series. Treat/post must be 0 or 1; treat must be constant within each entity, and entity-time pairs must be unique.

The node fits the observed Treat × Post TWFE effect with entity-clustered inference, then randomly reallocates treated-group membership across entities while keeping the treated count. Configure repetitions (10–2000, default 100) and a nonnegative seed (default 42); identical inputs and seed reproduce the same result.

Outputs result contain the observed coefficient, mean and standard deviation of permutation coefficients, valid permutation count and randomization p-value, or a structured unavailable reason. At least ten valid permutations are required. Cancellation is checked between permutations. This is a fake-group placebo test, not event-study estimation.

Configuration now exposes `constant` (true), `covariance` (cluster), and optional `use_observed_coefficient`/`observed_coefficient` (false/0). An explicit observed coefficient changes the randomization reference threshold; it does not refit the supplied observed effect. Without it the observed TWFE coefficient is fitted. Treat/post names and choices are included in the report. The ATT label refers to the TWFE treatment-by-post coefficient; causal ATT interpretation still requires appropriate DID identification assumptions.
