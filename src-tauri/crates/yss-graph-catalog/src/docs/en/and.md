# And (&&)

Logical conjunction on two booleans:

$$
\text{Result} = A \land B
$$

| $A$   | $B$   | Result |
| ----- | ----- | ------ |
| false | false | false  |
| false | true  | false  |
| true  | false | false  |
| true  | true  | true   |

## Usage

Combine multiple Boolean masks before filtering or another downstream data transformation. Chain comparisons with **Equal** into **A** and **B**.
