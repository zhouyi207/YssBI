# Drop Rows

Removes rows matching a condition. For example, `amount < 0` removes records with negative amounts, retaining other records and all columns.

Connect a DataFrame to Source and choose a column, operator, and comparison value in Predicate. Only true results are removed; false and NULL results are retained. Use the Is Null condition to remove missing values. Null checks do not require a comparison value.

Result is a new DataFrame; the source dataset is unchanged. This node removes rows by condition, not by row number. Use Filter Rows to retain matching rows.
