# Logical Comparison

Less, Less or Equal, Greater and Greater or Equal accept Numeric or Text inputs with matching meanings. Numeric comparisons retain integer precision. Text uses case-sensitive lexical order without numeric parsing or locale collation.

Two scalars return a Binary scalar. Any series input produces element-wise Binary results, broadcasting a scalar on either side. Materialized series must have equal lengths. Lazy series must share the same relation row domain and cannot mix with materialized lists lacking alignment information.

A missing operand produces a missing result. Non-finite numbers and unsupported representations fail. Categories, ordinal codes and identifiers are not implicitly sorted; datetime ordering is not provided by these nodes.

Lazy comparisons execute when consumed, preserve alignment, and do not modify the dataset.

Parameters in Detail selects `exact` (default, as described above) or `tolerance` (Numeric only). Tolerance mode shows absolute tolerance (default 1e-12) and relative tolerance (default 1e-9). Both must be finite and nonnegative; lossy Float64 conversions are rejected.

When `abs(a-b) <= atol + rtol * max(abs(a), abs(b))`, values are treated as equal: `<` and `>` return false, while `<=` and `>=` return true. Outside tolerance, ordinary numeric ordering applies. Use matching settings across all six comparisons for consistent results. Approximate equality is not transitive and must not define sorting or grouping equivalence.
