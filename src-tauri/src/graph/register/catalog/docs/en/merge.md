# Merge

Join multiple execution branches into one. When **any** **In** pin fires, execution continues on **Out**.

## Pins

| Pin | Direction | Description |
|-----|-----------|-------------|
| **In** *n* | Exec input (repeatable) | Any incoming branch; default two inputs, add more as needed |
| **Out** | Exec output | Fires once after any **In** triggers |

## Usage

Wire **True** and **False** paths from **Branch** into separate **In** pins, then connect **Out** to the shared downstream chain. Only the path that actually ran will trigger **Merge**; **Out** runs once per completed branch.
