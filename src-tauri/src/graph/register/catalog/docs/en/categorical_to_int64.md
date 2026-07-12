# Categorical to Int64

Parses category labels to `DataSeries<Int64>`; labels must be valid integers.

## Inputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | Input `DataSeries<Categorical>` or Enum |

## Outputs

| Pin | Description |
|-----|-------------|
| **DataSeries** | `DataSeries<Int64>`; invalid labels → null |

## Usage

Recover numeric codes from factor columns. Non-integer category names fail parse and become null.
