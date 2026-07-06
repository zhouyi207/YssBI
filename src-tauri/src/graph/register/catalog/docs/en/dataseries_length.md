# DataSeries Length

Returns the number of elements in a **DataSeries** as an **Int64** scalar.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **DataSeries** | Input | Series of any element type |
| **Length** | Output | Observation count (`Int64`) |

## Usage

Use to verify sample size, drive loop conditions, or combine with **Int Range**. Length follows the underlying Polars series row count.
