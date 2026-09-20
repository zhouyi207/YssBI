# Get DataSeries

Extract one column from a **DataFrame** as a **DataSeries**.

Preserves the column type, semantic metadata and row domain. Selecting a column returns a lazy reference and does not scan data.

## Usage

Connect a DataFrame and choose an existing column in the column parameter. The output can feed comparison, transformation or statistical nodes.
