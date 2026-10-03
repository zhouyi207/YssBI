# Cumulative Statistics

Connect a Numeric Series. Operation defaults to sum; mean, min, max and sample standard deviation (ddof=1) are available. Connect an equally long optional Context to choose Partition columns and Order columns; context rows pair with the series by current position. Empty selections use one partition and the current row order; equal keys retain source order.

Each row uses its partition's rows from the beginning through the current row. Null values are ignored; a window with no valid values returns Null. Results are Float64 Numeric and restored to the source row order.
