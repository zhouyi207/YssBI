# Equal (==)

Tests whether two values are equal:

$$
\text{Result} = (A = B)
$$

Compares scalars or series with matching meanings, supporting all seven basic meanings. Numeric values compare exactly across signed integer, unsigned integer and floating representations without epsilon or lossy promotion. Text compares original case-sensitive values without numeric parsing. Categorical and Ordinal compare codes rather than display labels.

Two scalars return a Binary scalar. Any series input returns element-wise Binary results with scalar broadcasting on either side. Materialized series must have equal lengths; lazy series must share one relation row domain and cannot mix with unaligned materialized lists. A missing operand produces a missing result. Non-finite numbers and unsupported representations fail.

## Usage

Produce comparison results for downstream computation. Use Whole Value Equal to compare complete lists or records and return one result.
