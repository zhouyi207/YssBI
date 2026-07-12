# Get DataSeries

Extract one column from a **DataFrame** as a **DataSeries**.

**Int64** / **Date** columns are tagged as unaligned time series (`Unaligned`) for downstream **TS** nodes.

## Pin

| Pin | Direction | Description |
|-----|-----------|-------------|
| **DataFrame** | Input | Source table |
| **Column Name** | Input | Column to extract (`String`) |
| **DataSeries** | Output | Single-column series; element type matches the source column |

## Usage

Connect **Get DataFrame** or a database query result, and set **Column Name** (or wire a **String** constant). Pipe the output into comparison, transform, plot, or econometric nodes.
