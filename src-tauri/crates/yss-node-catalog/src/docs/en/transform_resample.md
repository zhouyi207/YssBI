# Resample Time

Select a Datetime Time column, optional grouping Keys and the value Columns. Unit defaults to day; choose year, quarter, month, week, day, hour, minute or second. Aggregation defaults to mean.

Result contains the bucketed time, keys and column_operation values. Numeric aggregations ignore Null; count accepts any value type. Time values remain timezone-free calendar values. This operation aggregates observed buckets; alignment is used to insert missing times.
