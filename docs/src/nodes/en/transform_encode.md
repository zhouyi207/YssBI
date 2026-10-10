# Generate Indicators

Connect a Categorical, Ordinal or Binary Series. Declare Category values as exact original-code text and distinct Output names in matching order. Drop reference indicator defaults to false; when enabled, Reference column must name one declared output column.

Result is a DataFrame of Binary indicator columns in the original row domain. Present values equal to a category produce true, other present values false, and Null inputs produce Null. Categories are declared before execution; no data scan discovers output columns.
