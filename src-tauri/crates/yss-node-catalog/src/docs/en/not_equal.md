# Not Equal (!=)

Configuration in Detail defaults to `exact`. Choose `tolerance` for Numeric inputs to show absolute tolerance (default 1e-12) and relative tolerance (default 1e-9). Both must be finite and nonnegative; lossy Float64 conversions are rejected. This mode returns false when `abs(a-b) <= atol + rtol * max(abs(a), abs(b))`, and true otherwise. Null remains null. Use matching settings with Equal to obtain complementary non-null results. The exact mode behaves as described below.

Tests whether two values differ:

$$
\text{Result} = (A \neq B)
$$

Uses the same element-wise comparison and broadcasting rules as Equal. Two scalars return a Binary scalar; any series input returns a Binary series. Non-null results are inverted; null remains null. Numeric representations compare exactly and text is never implicitly parsed as a number. Materialized series require equal lengths; lazy series require the same relation row domain.

## Usage

Produce a Boolean mask for filters or downstream data transformations when values differ.
