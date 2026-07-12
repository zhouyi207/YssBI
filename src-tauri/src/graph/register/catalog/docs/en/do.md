# Do

Pass-through control-flow node. When execution reaches **In**, it immediately continues on **Out** with no side effects.

## Pins

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** | Exec input | Incoming execution flow |
| **Out** | Exec output | Continues after **In** fires |

## Usage

Use **Do** to extend an exec chain, merge branch paths before **Merge**, or keep **Sequence** layouts readable. Functionally similar to **Print** without logging.
