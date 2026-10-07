# Standardize DataSeries

Computes z = (x - mean) / standard_deviation. Outputs the standardized series, mean, and sample standard deviation (ddof = 1). Null positions are preserved and excluded from the statistics. Fewer than two non-null values, zero variance, or non-finite statistics fail. Connect both scalar outputs to Inverse Standardize.

The standardized output is a lazy series retaining the input row domain, including for literal series. Only aggregate statistics are read for the two scalar outputs. Floating-point roundtrips can introduce rounding error; use Equal in tolerance mode with suitable tolerances to validate them.
