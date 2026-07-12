# Categorical to Float64

Parses category labels to `DataSeries<Float64>`; labels must parse as floats.

## Inputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Input `DataSeries<Categorical>` or Enum |

## Outputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | `DataSeries<Float64>`; invalid labels → null |

## Usage

Convert ordered factor levels stored as numeric strings back to continuous values for math nodes.
