# String to Float64

Parses a `DataSeries<String>` to `DataSeries<Float64>` element-wise via Polars cast.

## Inputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Input `DataSeries<String>` |

## Outputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Output `DataSeries<Float64>`; unparseable values → null |

## Usage

Import numeric text columns before math or **OLS**. Clean non-numeric tokens upstream to avoid silent nulls.
