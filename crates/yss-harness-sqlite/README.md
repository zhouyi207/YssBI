# Harness SQLite store

> Status: Current
> Scope: SQLite persistence adapter for current statistical Harness records
> Canonical owners: [connection and schema admission](src/lib.rs), [schema](src/schema.rs), and record-specific modules in src/
> Update when: Persistence, schema admission, identity checks, transaction or error behavior changes

`SqliteHarnessStore` implements the neutral persistence ports from
[Harness Contract](../yss-harness-contract/src/persistence.rs).
[Harness Core](../yss-harness-core/README.md) owns session, turn, workflow, invocation,
approval and source policy. The adapter stores their current records, enforces physical
identity and concurrency checks, and supplies ordered queries. It does not own native
panels, model execution, resource authorization or a second workflow state machine.

`connect` opens `db/statistical-harness.sqlite` below the supplied app directory using
SQLx's native path API, with WAL and foreign keys. Each store has one pooled connection.
The store and pool follow their Arc owner lifetime in Application. Test support can use
an in-memory database. The adapter creates its complete current schema in one transaction
when the database is empty; an existing schema must match exactly. It rejects incompatible
schema or records instead of rewriting history or introducing migration/version machinery.

JSON contains the Harness record; indexed identity/state columns are derived write values.
Invocation completion must match stored idempotency key, invocation ID and session ID.
Turn updates must match their ID and original session. An unmatched update returns
`NotFound` and cannot replace another record's payload. Approval insertion stores the
record's consumption time in both JSON and the nullable SQLite index; times that cannot
fit SQLite's integer carrier fail as `InvalidRecord`. Consumption uses a conditional
update so only one caller can succeed.

Session deletion removes its turns, events, tool invocations, workflow runs and approval
grants in the same transaction as the session. Failure rolls back the complete removal;
other sessions, shared workflow definitions and knowledge remain unchanged. Core owns
authorization and excludes active turns or unfinished workflows before calling this port.

Event append reserves the SQLite writer with `BEGIN IMMEDIATE` before reading the head,
then inserts and commits the next sequence. Failed appends consume no sequence, including
writers using separate store connections. Workflow definitions cannot be replaced at the
same ID/version. Run creation requires revision zero; updates compare the expected
revision and return the committed next revision. Conflicts do not overwrite current state.

Knowledge replacement/deletion runs in one transaction. Source/document ownership and
duplicate IDs are checked; cross-source collisions roll back the complete replacement.
The store's private in-memory generation invalidates its prepared index snapshots, including
cancelled commit futures. Snapshot reads acquire the sole connection before checking that
generation. Application shares one store for knowledge writes and reads; the counter is
not a database-wide change monitor. Core rechecks current sources and citations.

Database, uniqueness, missing-record and codec failures map to the neutral persistence
failure contract. This adapter never repairs malformed data through compatibility decoding.

Focused validation from the repository root:

```sh
cargo test -p yss-harness-sqlite --lib
cargo test -p yss-harness-core --lib
cargo clippy -p yss-harness-sqlite --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-harness-sqlite --check
```

Persistence changes also assess Application initialization and actual Harness recovery,
approval, knowledge and invocation consumers. Native Assistant behavior uses manual acceptance.
