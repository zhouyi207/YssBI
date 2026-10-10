# DataSeries Sum

Sums non-null numeric values. Empty/all-null input returns zero. Integer sums retain an exact signed or unsigned 64-bit integer result; other numeric inputs use Float64. Overflow, non-finite values and lossy numeric conversions fail. Only the aggregate result is read; the input series is not collected.
