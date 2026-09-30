# Backward Fill

Connect any Series. Optional Context allows grouping and ordering columns from the same row domain. Empty selections use one group and the current row order; equal keys retain source order.

For each missing value, use the next non-null value in its group, including the current row. Trailing missing values remain Null. Output preserves element type and metadata, and is restored to source row order.
