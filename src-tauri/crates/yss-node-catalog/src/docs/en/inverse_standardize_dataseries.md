# Inverse Standardize DataSeries

Connect standardized, mean and standard_deviation inputs. Computes x = standardized * standard_deviation + mean, preserving Null positions. The standard deviation must be positive and finite. Outputs a Float64 series.

Relation inputs retain their original row domain and evaluate lazily; in-memory inputs remain in-memory series. Floating-point restoration need not be bitwise equal to the original data. Use Equal in tolerance mode with suitable tolerances to check the roundtrip.
