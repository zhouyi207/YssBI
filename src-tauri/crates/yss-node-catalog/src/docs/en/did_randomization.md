# DID Fake-group Randomization

Connect aligned finite response, optional predictors, entity, time, treat and post series. Treat/post must be 0 or 1; treat must be constant within each entity, and entity-time pairs must be unique.

The node fits the observed Treat × Post TWFE effect with entity-clustered inference, then randomly reallocates treated-group membership across entities while keeping the treated count. Configure repetitions (10–2000, default 100) and a nonnegative seed (default 42); identical inputs and seed reproduce the same result.

Outputs result/report contain the observed coefficient, mean and standard deviation of permutation coefficients, valid permutation count and randomization p-value, or a structured unavailable reason. At least ten valid permutations are required. Cancellation is checked between permutations. This is a fake-group placebo test, not event-study estimation.
