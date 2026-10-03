# Describe

Describe each supported column of a DataFrame or DataSeries and return a structured Result. Expand Result in the node’s Details, then expand a column to view its statistics.

## Input

**Data**: one DataFrame, or a Numeric, Categorical, Ordinal or Binary DataSeries.

No parameters are required. A DataFrame describes all supported columns in source order; a DataSeries describes itself and produces one column summary.

Text, Datetime and Identifier columns are excluded from the summary. Computation requires at least one supported column. Categorical, Ordinal and Binary values retain their meaning even when stored as integer codes.

## Output

`result.columns` is a JSON object keyed by the original column names, with each column’s descriptive statistics included directly. Each entry contains `position` (the one-based input order among supported columns), `semantic` (semantic type), `count` (non-null count) and `missing` (null count), together with the applicable metrics:

| Input semantics              | Metrics                                             |
| ---------------------------- | --------------------------------------------------- |
| Numeric                      | `mean`, `std`, `min`, `q25`, `median`, `q75`, `max` |
| Categorical, Ordinal, Binary | `unique`, `categories`                              |

Each column returns only fields appropriate to its semantics. Numeric excludes categorical metrics; Categorical, Ordinal and Binary exclude numeric metrics such as the mean, standard deviation and quantiles.

Numeric metrics ignore nulls. `std` is the sample standard deviation with the non-null sample count minus one as denominator. Quantiles interpolate linearly at zero-based sorted position `(n−1)p`, where `n` is the non-null count.

Category counts exclude nulls. Empty strings are valid categories.

`categories` directly includes every observed non-null category, keyed by consecutive positions starting at `"1"`. Ordinal follows the declared level order; other semantics sort by original value ascending. Each entry contains:

| Field        | Meaning                                                                                                         |
| ------------ | --------------------------------------------------------------------------------------------------------------- |
| `value`      | Original category value; Decimal values and integers beyond JavaScript's exact range are returned as exact text |
| `label`      | Declared category label; omitted when no label is available                                                     |
| `frequency`  | Count of this category                                                                                          |
| `proportion` | This category's frequency divided by the non-null count                                                         |

The Result panel shows the complete per-column statistics and category frequencies directly in JSON. Details displays the same result in `position` order; expand Category frequencies within a categorical column to inspect each category.

Applicable but undefined metrics are Null, including standard deviation with fewer than two valid observations. With no non-null categories, `unique` is 0, `categories` is an empty object. Non-finite values or arithmetic results fail.
