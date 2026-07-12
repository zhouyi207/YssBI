# Float64 to Int64

Casts `DataSeries<Float64>` to `DataSeries<Int64>` (truncates toward zero).

## Inputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Input `DataSeries<Float64>` |

## Outputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Truncated integers; out-of-range or non-finite → null (Polars) |

## Usage

Quantize continuous series to integer bins or IDs. Expect nulls for NaN/Inf or values outside `i64` range.
