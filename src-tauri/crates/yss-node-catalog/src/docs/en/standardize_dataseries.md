# Standardize DataSeries

Computes z = (x - mean) / standard_deviation. Outputs the standardized series, mean, and sample standard deviation (ddof = 1). Null positions are preserved and excluded from the statistics. Fewer than two non-null values, zero variance, or non-finite statistics fail. Connect both scalar outputs to Inverse Standardize.

Relation inputs retain their original row domain through a lazy, null-preserving expression. Standardization scans the input to compute statistics first; inverse standardization reuses them. In-memory inputs remain in-memory series. Floating-point roundtrips can introduce rounding error; use Equal in tolerance mode with suitable tolerances to validate them.
