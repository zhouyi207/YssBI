# Logarithm

Computes `result = log_base(value)`. The left Value input is the argument and the right Base input is the logarithm base. Both accept numeric literals or connections. Float64 results include `log_2(8) = 3` and `log_10(100) = 2`.

The value must be positive; the base must be positive and different from 1. Bases between 0 and 1 are valid. Two scalars return a scalar; series inputs produce element-wise results and support scalar broadcasting on either side.

Series require equal lengths and pair by current position; database and in-memory inputs may be mixed. Nulls, invalid domains, non-finite inputs/results, and integers that cannot widen exactly to Float64 fail. Lazy computation executes when consumed without modifying the source dataset.
