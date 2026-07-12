# Float64 to String

Formats a `DataSeries<Float64>` as `DataSeries<String>` via Polars cast.

## Inputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Input `DataSeries<Float64>` |

## Outputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | String form of each float (Polars default formatting) |

## Usage

Display numeric results as text or feed **String to Categorical**. Null and non-finite values follow Polars cast rules.
