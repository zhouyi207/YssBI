# Drop Columns

Removes selected columns from the input DataFrame while preserving the order, types, and row data of the remaining columns. For input `a, b, c`, dropping `["b"]` produces `a, c`.

Connect a DataFrame to Source and select at least one existing column in Columns. Names must be unique and at least one column must remain. Other columns added upstream are retained automatically.

Result is a new DataFrame; the source dataset is unchanged. Use Select Columns to choose retained columns or reorder them.
