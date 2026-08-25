# And (&&)

Logical conjunction on two booleans:

$$
\text{Result} = A \land B
$$

| $A$ | $B$ | Result |
|-----|-----|--------|
| false | false | false |
| false | true | false |
| true | false | false |
| true | true | true |

## Inputs

| Pin | Description |
|-----|-------------|
| **A** (optional) | First `Boolean` |
| **B** (optional) | Second `Boolean` |

## Outputs

| Pin | Description |
|-----|-------------|
| **Result** | `Boolean`: both inputs must be true |

## Usage

Combine multiple conditions before **Branch** or **Set Variable**. Chain comparisons with **Equal** into **A** and **B**.
