# Int64 to Float64

Casts `DataSeries<Int64>` to `DataSeries<Float64>` with exact integer representation where possible.

## Inputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Input `DataSeries<Int64>` |

## Outputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | `DataSeries<Float64>` with widened numeric type |

## Usage

Prepare integer columns for **Ln**, **Divide**, or **OLS** without losing integer values within `f64` precision. Null stays null.
