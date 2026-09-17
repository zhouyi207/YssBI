# Or (||)

Logical disjunction on two booleans:

$$
\text{Result} = A \lor B
$$

| $A$   | $B$   | Result |
| ----- | ----- | ------ |
| false | false | false  |
| false | true  | true   |
| true  | false | true   |
| true  | true  | true   |

Accepts Binary scalars, materialized series, and lazy series. Series execute element-wise; AND/OR broadcast a scalar on either side. Materialized series require equal lengths and lazy series require the same relation row domain. Lazy results execute when read or consumed downstream without modifying the source dataset. Three-valued logic applies: `false AND null = false`, `true AND null = null`, `true OR null = true`, `false OR null = null`, and `NOT null = null`. These nodes combine data conditions and do not guarantee skipping upstream node evaluation.

## Usage

Accept any of several conditions (e.g. threshold OR flag set). Feed comparison nodes into **A** and **B**.
