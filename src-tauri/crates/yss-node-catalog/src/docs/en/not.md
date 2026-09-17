# Not (!)

Logical negation:

$$
\text{Result} = \lnot A
$$

| $A$   | Result |
| ----- | ------ |
| false | true   |
| true  | false  |

Accepts Binary scalars, materialized series, and lazy series. Series execute element-wise; AND/OR broadcast a scalar on either side. Materialized series require equal lengths and lazy series require the same relation row domain. Lazy results execute when read or consumed downstream without modifying the source dataset. Three-valued logic applies: `false AND null = false`, `true AND null = null`, `true OR null = true`, `false OR null = null`, and `NOT null = null`. These nodes combine data conditions and do not guarantee skipping upstream node evaluation.

## Usage

Invert a Boolean mask before filtering or another downstream data transformation.
