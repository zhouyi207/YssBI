# GroupBy

Connect a DataFrame to **Source** and choose one or more unique **Group Keys**. You can enter
key names before connecting; a known input schema supplies choices and validates those names.

Running the node produces a **Grouped DataFrame** containing the source and its keys. Inspecting
it shows the key and source column names. Connect this output to **Apply to Groups** or
**Transform Groups** to call a graph function. Creating or connecting the node does not run it.

Null keys form groups. Groups sort by keys ascending, with null first; rows within a group retain
their source order. Group inputs include the key columns. Explicitly running GroupBy retains its
source result, which subsequent manual steps can reuse.

For fixed summaries such as count, sum and mean, use **Grouped Aggregation**, which computes
those aggregates directly from its input DataFrame.
