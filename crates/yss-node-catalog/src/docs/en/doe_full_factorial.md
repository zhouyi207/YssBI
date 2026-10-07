# DOE: full factorial design

No input. **result** contains a compact summary; **design** is the complete coded table with `run` and `factor1`…`factorK`. Run and level codes are one-based.
Changing the factor count updates downstream column choices. Generated order is not a randomized field schedule: arrange randomization, replication and blocking for the actual experiment.

There is no fixed row or factor cap. Integer overflow, loss of exact numeric integer representation and execution resource budgets reject an infeasible design instead of truncating it.

**factors** defaults to 3; **levels** defaults to 2 and must be at least 2.
Enumerates all `levels^factors` combinations with the first factor varying fastest. All factors have the same level count.
Codes identify discrete settings; map them to physical values before use. Run count grows exponentially. This node does not automatically choose fractional, screening or optimal designs.

[参考 / Reference](https://www.itl.nist.gov/div898/handbook/pri/section3/pri339.htm)
