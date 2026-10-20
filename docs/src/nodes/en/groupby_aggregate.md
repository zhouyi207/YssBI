<!-- nodes: yssbi.dataframe.groupby -->
# Grouped Aggregation

Groups by one or more keys, including null keys. Always outputs row_count. Choose columns independently for count, sum, mean, min, max, sample standard deviation and median; names are column_operation. Count excludes null; numeric aggregates require Numeric semantics and ignore null. All-null numeric aggregates return null. Duplicate output names are rejected. Output is a pageable DataFrame sorted by keys.
