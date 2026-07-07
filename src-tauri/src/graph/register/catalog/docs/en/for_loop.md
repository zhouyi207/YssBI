# For Loop

Execute a body branch a fixed number of times. **Index** outputs the current iteration (0-based) on each pass.

## Pins

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** | Exec input | Starts the loop; wire **Body** back here after each iteration |
| **Count** | Input (optional) | `Int64` iterations; default 1 |
| **Index** | Output | Current iteration index (`0` … `Count - 1`) |
| **Body** | Exec output | Runs while iterations remain |
| **Completed** | Exec output | Runs after all iterations finish |

## Wiring

Connect the last node in **Body** back to **In** (e.g. via **Do**). When **Body** finishes, execution re-enters **For Loop**, increments **Index**, and continues until **Count** is reached.

## Example

`Count = 3` → **Index** emits `0`, `1`, `2`; then **Completed** fires once.
