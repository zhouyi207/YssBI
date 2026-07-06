# Or (||)

Logical disjunction on two booleans:

$$
\text{Result} = A \lor B
$$

| $A$ | $B$ | Result |
|-----|-----|--------|
| false | false | false |
| false | true | true |
| true | false | true |
| true | true | true |

## Inputs

| Pin | Description |
|-----|-------------|
| **A** (optional) | First `Boolean` |
| **B** (optional) | Second `Boolean` |

## Outputs

| Pin | Description |
|-----|-------------|
| **Result** | `Boolean`: true if either input is true |

## Usage

Accept any of several conditions (e.g. threshold OR flag set). Feed comparison nodes into **A** and **B**.
