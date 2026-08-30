# Get DataSeries

Extract one column from a **DataFrame** as a **DataSeries**.

**Int64** / **Date** columns are tagged as unaligned time series (`Unaligned`) for downstream **TS** nodes.

## Usage

Connect **Get DataFrame** or a database query result, and set **Column Name** (or wire a **String** constant). Pipe the output into comparison, transform, plot, or econometric nodes.
