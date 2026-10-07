# Orthogonal design (equal levels)

No input. **result** contains a compact summary; **design** is the complete coded table with `run` and `factor1`…`factorK`. Run and level codes are one-based.
Changing the factor count updates downstream column choices. Generated order is not a randomized field schedule: arrange randomization, replication and blocking for the actual experiment.

There is no fixed row or factor cap. Integer overflow, loss of exact numeric integer representation and execution resource budgets reject an infeasible design instead of truncating it.

**factors** defaults to 3 and **levels** to 2 (minimum 2). For prime q, a linear finite-field construction uses the smallest m with `(q^m−1)/(q−1) ≥ factors`, producing q^m runs.
Three binary factors yield L4; four ternary factors yield L9. Every pair of distinct factors contains every ordered level pair equally often.

For composite level counts, full factorial enumeration preserves this balance with `levels^factors` runs, without claiming a minimal orthogonal array.
`method` identifies the construction. Mixed-level arrays and interaction-column allocation are not provided; main effects can remain aliased with interactions.

[参考 / Reference](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.qmc.LatinHypercube.html)
