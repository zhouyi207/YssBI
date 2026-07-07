# Switch

Dispatch execution by integer **Selector**. Matching case index triggers the corresponding **Case** output; otherwise **Default** runs.

## Pins

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** | Exec input | Incoming execution |
| **Selector** | Input (optional) | `Int64` case index; default 0 |
| **Case** *n* | Exec output (repeatable) | Fires when `Selector == n`; default 2 cases |
| **Default** | Exec output | Fires when selector is negative or ≥ case count |

## Usage

Use **Switch** instead of long **Branch** chains for discrete integer modes (plot type, model family, etc.).
