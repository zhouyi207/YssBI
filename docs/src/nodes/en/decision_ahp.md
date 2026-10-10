# AHP judgment weights

Connect the columns of one square judgment matrix to **criteria**.
Row and column order must refer to the same criteria. A(i,j) is the importance
of row i relative to column j: entries are positive, diagonal entries are 1,
and opposite entries are reciprocals (relative log tolerance 1e−8).
Missing and nonfinite data are rejected.

Weights use the normalized positive principal eigenvector.
**result** reports the principal eigenvalue, CI = (λ − n)/(n − 1),
RI, CR = CI/RI and whether CR ≤ 0.1. CI is 0 for one criterion.
**weights** is a reusable vector in criterion order.

**random_index** = 0 uses the Tummala–Ling reference table for orders 3–15;
a positive value supplies RI explicitly. If RI is unavailable, CR and its
judgment are null; weights and CI still compute. No matrix order cap is imposed.
This node analyzes one hierarchy level; aggregate expert judgments with a
geometric mean and combine separate hierarchy levels explicitly.

Reference: [PyMCDM AHP and RI convention](https://pymcdm.readthedocs.io/en/latest/pymcdm.weights.html).
