# Equal (==)

Tests whether two values are equal:

$$
\text{Result} = (A = B)
$$

Compares scalars or series with matching meanings. Select a comparison mode under Configuration in Detail:

- `exact` (default): supports all seven basic meanings. Numeric values compare exactly across signed integer, unsigned integer and floating representations without epsilon or lossy promotion. Text compares original case-sensitive values without numeric parsing. Categorical and Ordinal compare codes rather than display labels.
- `tolerance`: Numeric only, using the symmetric rule `abs(a-b) <= atol + rtol * max(abs(a), abs(b))`. This mode shows absolute tolerance (default 1e-12) and relative tolerance (default 1e-9). Both must be finite and nonnegative; lossy Float64 conversions are rejected. Zero tolerances compare the converted values exactly.

Two scalars return a Binary scalar. Any series input returns element-wise Binary results with scalar broadcasting on either side. Materialized series must have equal lengths; lazy series must share one relation row domain and cannot mix with unaligned materialized lists. A missing operand produces a missing result. Non-finite numbers and unsupported representations fail.

## Usage

Produce comparison results for downstream computation. For standardize/inverse-standardize roundtrips, choose tolerance mode and tolerances appropriate to the data scale. Input values are unchanged. All six comparisons support the same configuration; with matching settings, Not Equal negates non-null Equal results.
