# Pivot

Choose Keys, Category column and Value column. Enter Category values as exact original-code text and give each a distinct Output name in the same order. Only declared categories become columns; output names cannot collide with keys.

Aggregation defaults to sum. Sum, mean, min and max require Numeric values and ignore Null; count counts non-null values. Empty numeric groups return Null. Result is ordered by keys. Declaring output categories keeps the result schema fixed before execution.
