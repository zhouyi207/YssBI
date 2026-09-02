# Data Store

`graphProjectionStore` holds normalized, read-only Rust editor projections. Its graph buckets are
replaced as complete projections and are not a second authoritative graph document.

Unsaved Graph documents and history belong to `features/core/graphDraft`. Project/resource,
database, variable, and graph metadata stores keep their own scoped projections. Cross-store load,
save, and reset workflows belong to the Application layer.
