# Power

Computes `result = base ^ exponent`. Both inputs accept numeric literals or connections. Results use Float64, for example `2 ^ 3 = 8`, `9 ^ 0.5 = 3`, and `2 ^ -1 = 0.5`. Set the base to e for an exponential function.

Two scalars return a scalar; any series input produces element-wise results with scalar broadcasting. Series require equal lengths and pair by current position; database and in-memory inputs may be mixed.

Negative bases require integer exponents. A zero base requires a positive exponent; `0 ^ 0` is rejected. Nulls, non-finite inputs, integers that cannot widen exactly to Float64, and overflow fail. Lazy values are checked when consumed and do not modify the source dataset.
