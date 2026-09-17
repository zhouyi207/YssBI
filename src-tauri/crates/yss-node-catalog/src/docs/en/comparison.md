# Logical Comparison

Less, Less or Equal, Greater and Greater or Equal accept Numeric or Text inputs with matching meanings. Numeric comparisons retain integer precision. Text uses case-sensitive lexical order without numeric parsing or locale collation.

Two scalars return a Binary scalar. Any series input produces element-wise Binary results, broadcasting a scalar on either side. Materialized series must have equal lengths. Lazy series must share the same relation row domain and cannot mix with materialized lists lacking alignment information.

A missing operand produces a missing result. Non-finite numbers and unsupported representations fail. Categories, ordinal codes and identifiers are not implicitly sorted; datetime ordering is not provided by these nodes.

Lazy comparisons execute when consumed, preserve alignment, and do not modify the dataset.
