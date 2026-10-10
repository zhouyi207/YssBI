# Select Columns

Keep selected columns from the input DataFrame in the order specified by Columns. For example, given `a, b, c`, selecting `["c", "a"]` produces columns `c, a` in that order.

Connect the DataFrame to Source and specify at least one existing column name in Columns. Duplicate names are not allowed. Result preserves the original rows and row order.

Use this node to select and reorder columns. Use Filter Rows to filter rows and Rename DataFrame to rename columns. The operation produces a derived result without modifying the source dataset.
