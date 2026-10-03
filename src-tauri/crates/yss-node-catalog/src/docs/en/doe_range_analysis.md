# Range analysis (factor-level effects)

Connect a numeric **response** and aligned categorical **factors**, one factor
per input. Each row is an experimental run. Missing categories and nonfinite or
missing responses are rejected. Numeric factor codes are categories, not slopes.

**levels** reports the count, response sum K and mean at each observed level.
**result** reports each factor's raw range R (largest minus smallest level mean)
and every best level, maximizing by default or minimizing when **maximize** is
off. Floating-point indistinguishable means are treated as ties. One-based factor
and level indices follow `factor_names` and `level_labels`, preserving exact labels.

The report checks equal replication within factors, equal numbers of levels
across factors, and pairwise orthogonality using the exact contingency counts.
Pairwise orthogonality is null for a single factor. Unbalanced/nonorthogonal
data still produce descriptive means, but factor comparisons may be confounded.
Raw ranges have no mixed-level range-bias correction and no significance test.

The selected combination need not have been observed and is not a validated
optimum; interactions and confirmation runs require separate analysis.
All runs contribute to the summaries without a fixed row limit.
