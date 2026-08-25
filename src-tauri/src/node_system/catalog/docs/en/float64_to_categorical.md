# Float64 to Categorical

Encodes `DataSeries<Float64>` as `DataSeries<Categorical>` via string representation.

## Inputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Input `DataSeries<Float64>` |

## Outputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | `DataSeries<Categorical>` from string forms of each float |

## Usage

Bucket continuous values into labeled categories for plotting or fixed-effect style grouping. Format follows Polars float-to-string rules.
