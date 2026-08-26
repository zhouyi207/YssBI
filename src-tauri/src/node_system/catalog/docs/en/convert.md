# Convert

Converts a **scalar** value to another scalar type. Set the target type on the **Output** pin (type inference constrains **Input**).

## Supported scalar conversions

Rows = source type, columns = target type. ✓ = supported; — = use a dedicated **DataSeries** conversion node.

| From \ To | Boolean | Int64 | Float64 | String | Categorical | DataSeries |
|-----------|---------|-------|---------|--------|-------------|------------|
| **Boolean** | ✓ | ✓ (0/1) | ✓ (0.0/1.0) | ✓ | — | — |
| **Int64** | ✓ (≠0) | ✓ | ✓ | ✓ | — | — |
| **Float64** | ✓ (≠0) | ✓ (truncate) | ✓ | ✓ | — | — |
| **String** | ✓* | ✓* | ✓* | ✓ | — | — |
| **Null** | → false | → 0 | → 0.0 | → `"null"` | — | — |
| **Categorical** | — | — | — | — | — | — |
| **DataSeries** | — | — | — | — | — | — |

\*String parsing: Boolean accepts `true`/`false`, `1`/`0`, `yes`/`no`, `on`/`off` (case-insensitive); invalid strings error. Int64/Float64 use standard parse rules.

## Usage

Connect any scalar producer to **Input**, pick **Output** type, and wire downstream. For column-wise casts use **String to Float64**, **Int64 to Categorical**, etc.
