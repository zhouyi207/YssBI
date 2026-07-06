# Int64 to String

Formats a `DataSeries<Int64>` as `DataSeries<String>` via Polars cast.

## Inputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Input `DataSeries<Int64>` |

## Outputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Decimal string representation per element |

## Usage

Export integer keys as labels or prepare for **String to Categorical**. Null inputs stay null.
