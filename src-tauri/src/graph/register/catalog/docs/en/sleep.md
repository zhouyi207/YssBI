# Sleep

Pause execution for a duration before continuing.

## Pins

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** | Exec input | Starts the wait |
| **Duration** | Input (optional) | Seconds as `Float64`; default 1.0 when unconnected |
| **Out** | Exec output | Fires after the wait completes |

## Limits

Duration is clamped to **0–60** seconds per invocation.

## Usage

Insert **Sleep** between **Sequence** steps for staged runs, demos, or throttled logging. Connect **Duration** to a **Float64** constant or computed value.
