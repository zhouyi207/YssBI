# Not Equal (!=)

Tests whether two values differ:

$$
\text{Result} = (A \neq B)
$$

Compares scalar `Float64` operands. Output is a single `Boolean`.

## Inputs

| Pin | Description |
|-----|-------------|
| **A** (optional) | First `Float64` operand |
| **B** (optional) | Second `Float64` operand |

## Outputs

| Pin | Description |
|-----|-------------|
| **Result** | `true` if **A** and **B** are not equal |

## Usage

Negate equality checks in control flow. Pair with **Branch** to route when values differ.
