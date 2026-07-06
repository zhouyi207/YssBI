# Branch

Conditional control-flow node. When execution reaches **In**, exactly one of **True** or **False** exec outputs fires according to **Condition**.

## Pins

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** | Exec input | Incoming execution flow |
| **Condition** | Input (optional) | Boolean; when unconnected, treated as false |
| **True** | Exec output | Triggered when **Condition** is true |
| **False** | Exec output | Triggered when **Condition** is false |

## Usage

Wire **In** from an upstream exec pin (e.g. **Event Begin**, **Sequence**, or **Print**). Connect **Condition** to a boolean value or expression result. Attach the branch you want to run to **True** or **False**. Only the chosen path executes; the other remains idle.
