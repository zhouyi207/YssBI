# Float64 to Int64

Casts `DataSeries<Float64>` to `DataSeries<Int64>` (truncates toward zero).

## Usage

Quantize continuous series to integer bins or IDs. Expect nulls for NaN/Inf or values outside `i64` range.
