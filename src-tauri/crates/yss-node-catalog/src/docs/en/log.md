# Logarithm

Computes `result = log_base(value)`. The left Value input is the argument and the right Base input is the logarithm base. Both accept numeric literals or connections. Float64 results include `log_2(8) = 3` and `log_10(100) = 2`.

The value must be positive; the base must be positive and different from 1. Bases between 0 and 1 are valid. Two scalars return a scalar; series inputs produce element-wise results and support scalar broadcasting on either side.

Materialized series require equal lengths; lazy series require the same relation row domain. Nulls, invalid domains, non-finite inputs/results, and integers that cannot widen exactly to Float64 fail. Lazy computation executes when consumed without modifying the source dataset.
