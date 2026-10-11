# Transform Groups

Connect a **Grouped DataFrame** and select a graph function with exactly one DataFrame parameter
and a DataFrame return. The group table includes its keys. Transform combines the returned columns
in the original source row order, without automatically adding source or key columns.

The returned table must preserve every group row and its order. Selecting, renaming or computing
columns over those rows can preserve this relationship. Filtering, sorting, aggregation and
independently created tables cannot prove it, even when their row counts happen to match.

Every group must return the same column names, order, exact physical types and semantic
configurations. To return fewer, more or reordered rows, use **Apply to Groups**.

Empty input calls the function once with an empty source to determine its output structure, and
returns zero rows. Groups run sequentially with the caller's cancellation and budgets. Errors retain
the group and function location; a failed or cancelled run does not restore the previous output.
