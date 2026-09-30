# Inverse Standardize DataSeries

Connect standardized, mean and standard_deviation inputs. Computes x = standardized * standard_deviation + mean, preserving Null positions. The standard deviation must be positive and finite. Outputs a Float64 series.

The output is a lazy series retaining the input row domain, including for literal series. Floating-point restoration need not be bitwise equal to the original data. Use Equal in tolerance mode with suitable tolerances to check the roundtrip.
