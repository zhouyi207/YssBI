# Uniform design (centered Latin hypercube)

No input. **result** contains a compact summary; **design** is the complete coded table with `run` and `factor1`…`factorK`. Run and level codes are one-based.
Changing the factor count updates downstream column choices. Generated order is not a randomized field schedule: arrange randomization, replication and blocking for the actual experiment.

There is no fixed row or factor cap. Integer overflow, loss of exact numeric integer representation and execution resource budgets reject an infeasible design instead of truncating it.

Parameters are **factors** (3), **runs** (12), **candidates** (32), and **seed** (42).
Each factor uses every level from 1 to runs exactly once. The seed controls random permutations; the candidate with the smallest centered L2 discrepancy is retained.

Discrepancy uses normalized coordinates `(level−0.5)/runs`. `centered_l2_discrepancy` is squared, matching SciPy `qmc.discrepancy(method="CD")`; no square root is taken.
Map to physical bounds [a,b] using `a + (b−a)×(level−0.5)/runs`.

This finite candidate search does not guarantee global optimality or reproduce a named U table. Work scales as candidates×runs²×factors and observes cancellation/deadlines.

[参考 / Reference](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.qmc.discrepancy.html)
