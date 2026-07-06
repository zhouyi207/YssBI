# String to Int64

Parses a `DataSeries<String>` to `DataSeries<Int64>` element-wise via Polars cast.

## Inputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Input `DataSeries<String>` |

## Outputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Output `DataSeries<Int64>`; invalid parse → null |

## Usage

Convert ID or count columns stored as text. Fractional strings fail parse and become null.
