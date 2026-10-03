# Data Store

`graphProjection.ts` prepares normalized, read-only Rust graph sessions, entity buckets and result
summaries. It validates projections and shares unchanged branches; it owns no Zustand store.
`../resource/resourceStore.ts` publishes those candidates together with resource and document state.
Single-graph receipts, project snapshots, unload and reset use that same owner.

`graphMeta.ts` prepares the graph-kind and function-signature projection with structural sharing.
ResourceStore publishes it with resource names and revisions during project load, snapshot commit
and reset. Closing an editor releases the loaded graph frame while retaining indexed function metadata.

Current Graph documents, saved-content identity and history belong to Rust Project. DatabaseStore
keeps the database projection. Application owns receipt admission, request
lifecycles, project identity checks and coordination with Results, execution and workbench owners.
