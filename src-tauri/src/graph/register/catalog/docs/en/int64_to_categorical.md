# Int64 to Categorical

Encodes `DataSeries<Int64>` as `DataSeries<Categorical>` via string representation (same category pool as other cat casts).

## Inputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Input `DataSeries<Int64>` |

## Outputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | `DataSeries<Categorical>` with levels from decimal string forms |

## Usage

Treat integer codes as unordered factors for regression. Distinct integers become distinct categories.
