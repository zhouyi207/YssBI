# Sequence

Run multiple exec branches in a fixed order. After **In** fires, step outputs **Then 0**, **Then 1**, **Then 2**, … execute sequentially—each connected downstream chain runs before the next step starts.

## Usage

Connect **In** from **Event Begin** or a **Branch** path. Wire side effects (e.g. **Print**, **View**, data nodes with exec pins) to each **Then** step in the order you need. Steps run one after another, not in parallel—useful for staged logging, inspection, or ordered setup/teardown.
