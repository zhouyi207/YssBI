# Harness contracts

> Status: Current
> Scope: Platform-neutral Harness types and ports
> Canonical owners: `src/lib.rs` exports public contracts; the corresponding source modules own their values and conversions
> Update when: A shared Harness contract or its consumers change

`yss-harness-contract` defines the typed boundaries used by Harness Core,
Application, provider/storage adapters and the native desktop. It has no GUI,
Application or runtime dependency. Runtime authority and lifecycle rules belong
to [Harness Core](../yss-harness-core/README.md).

- `harness`, `agents` and `persistence` own model events, agent outcomes, sessions,
  ordered event envelopes and the tool ledger. These retain internal execution
  and consistency facts needed by their owners.
- `models` owns provider/model configuration and validation. The credential scope
  matches account ID, provider name and adapter; custom display names, endpoints
  and protocol edits within that scope do not select a different stored key.
  Application enforces this scope before saving or discovering with a saved key;
  desktop forms use the same predicate for connection readiness and feedback.

## Validation

Run from the repository root:

```sh
cargo clippy -p yss-harness-contract --lib --no-deps -- -D warnings
cargo check -p yss-desktop-gpui --bin yss-desktop-gpui
```
