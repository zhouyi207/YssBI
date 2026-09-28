# Time-series Alignment

Connect a DataFrame and select time_column. Time must be Int64, UInt64 (at most i64::MAX) or Date32 with no null or duplicate keys. Positive interval is measured in the integer time unit, or days for Date32.

Rows are sorted and gaps between the first and last observed times are filled. Every observed time must lie on the grid starting at the minimum time; off-grid observations are rejected. Original column order, physical types and metadata are retained; inserted non-time values are null. The output size is checked against the execution memory budget.
