# Drop NA Columns

Lazily removes candidate columns with any/all Null values. Columns are resolved on consumption over the entire input, before paging. Empty input retains columns. Output schema is data-dependent; static column selection/decomposition is unavailable.
