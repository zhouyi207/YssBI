# Standardize DataSeries

Computes z = (x - mean) / standard_deviation. Outputs the standardized series, mean, and sample standard deviation (ddof = 1). Null positions are preserved and excluded from the statistics. Fewer than two non-null values, zero variance, or non-finite statistics fail. Connect both scalar outputs to Inverse Standardize.
