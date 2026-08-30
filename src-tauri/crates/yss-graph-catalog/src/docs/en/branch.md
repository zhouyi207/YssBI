# Branch

Conditional control-flow node. When execution reaches **In**, exactly one of **True** or **False** exec outputs fires according to **Condition**.

## Usage

Wire **In** from an upstream exec pin (e.g. **Event Begin**, **Sequence**, or **Print**). Connect **Condition** to a boolean value or expression result. Attach the branch you want to run to **True** or **False**. Only the chosen path executes; the other remains idle.
