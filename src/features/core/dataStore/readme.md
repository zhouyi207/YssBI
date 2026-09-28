# Data Store

`graphProjectionStore` holds normalized, read-only Rust editor projections. Its graph buckets are
replaced as complete projections and are not a second authoritative graph document.

Current Graph documents, saved-content identity and history belong to Rust Project. Project/resource,
database, and graph metadata stores keep their own scoped projections. Cross-store load,
save, and reset workflows belong to the Application layer.
