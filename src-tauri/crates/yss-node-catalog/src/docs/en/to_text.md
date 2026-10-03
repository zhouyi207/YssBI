# To Text

Convert a scalar or series to Text. Scalars remain scalar; series retain row order and alignment. There are no parameters.

## Conversion rules

Format stored values rather than replacing Categorical or Ordinal codes with label text. Datetime values use their calendar representation while retaining wall time.

Empty strings remain empty strings. Nulls remain null rather than becoming the text `null`.

## Output and execution

The output meaning is fixed to Text. Materialized inputs are converted immediately; lazy series are checked when consumed. This node produces derived results without editing the source dataset.
