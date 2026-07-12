# While Loop

Repeat **Body** while **Condition** is true. **MaxIterations** caps total passes for safety.

## Pins

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** | Exec input | Starts or re-enters the loop; wire **Body** back here |
| **Condition** | Input (optional) | `Boolean`; unconnected defaults to false |
| **MaxIterations** | Input (optional) | `Int64` safety cap; default 1000 |
| **Body** | Exec output | Runs while condition is true and under the cap |
| **Completed** | Exec output | Runs when condition is false or cap is reached |

## Wiring

Connect the end of **Body** back to **In**. Each time **Body** completes, the node re-evaluates **Condition**.

## Limits

If **MaxIterations** is exceeded, the loop exits via **Completed** even if **Condition** stays true.
