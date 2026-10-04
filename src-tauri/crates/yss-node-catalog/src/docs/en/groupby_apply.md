# Apply to Groups

Connect a **Grouped DataFrame** and select a graph function with exactly one DataFrame parameter
and a DataFrame return. Each invocation receives one group's rows, including its key columns.
The function can return a different number of rows, including zero.

**Group Key Prefix** defaults to `group.` and can be empty. Apply prepends the original group keys
to each returned row. For an `industry` key and a returned `sales` column, the default output is
`group.industry, sales`. A returned column with the same name as a prefixed key is rejected;
change the prefix or rename that returned column.

Results concatenate in group order, preserving each returned table's order. Every group must return
the same column names, column order, exact physical types and semantic configurations. Apply checks
all groups; it does not fill missing columns or choose types from the first group alone.

Empty input calls the function once with an empty source to determine its output structure, then
returns zero rows. The function must support that empty input. Groups execute sequentially and
share the run's cancellation and budgets. A failure identifies its group and function; incomplete
combined output is not published.
