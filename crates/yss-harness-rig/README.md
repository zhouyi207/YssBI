# yss-harness-rig

> Status: Current
> Scope: Provider clients, native model turns, tool adapters, streaming recovery and working-context compaction
> Canonical owners: This crate owns the Rig adapter; Harness Core owns sessions, durable events, tool admission, receipts and resource observations
> Update when: Provider translation, sampling recovery, tool lifecycle or context compaction changes

`RigAgentDriver` implements the provider-neutral `AgentDriverPort` from
[Harness Contract](../yss-harness-contract/src/lib.rs). Application resolves model
settings and supplies the driver to [Harness Core](../yss-harness-core/README.md).
The adapter does not own project state or durable conversation history, and has
no dependency on Core, Application, a desktop framework or Tauri.

| Module | Responsibility |
| --- | --- |
| `provider` / `run_options` | Build protocol clients, list models and translate configured generation options |
| `messages` | Map model-facing messages into native call/result groups without changing persisted history |
| `tools` | Decode trusted tool schemas and call Core's capability and control ports |
| `driver` / `stream` / `recovery` | Run model calls, deliver ordered events, classify failures and recover interrupted sampling |
| `context` | Capture settled sampling boundaries and compact working context with verified prefix checkpoints |

Tools enter Core's existing admission and receipt lifecycle. The adapter retains
admitted tasks until their outcomes settle, including after cancellation; stopping
sampling does not discard a committed operation's receipt. Ordinary tool failures
return model feedback. Fatal lifecycle or persistence failures stop sampling.
Failure observations use fixed categories and structural counts. Unknown tool
names supplied by the provider are not included in logs.

Internal workflow operations have no callable model schema. Their current
receipts can still appear in conversation history, where the shared Contract
projection retains business facts without exposing consistency fields. Replaying
those receipts does not grant permission to invoke the internal operation.

Recovery reissues only a sampling request from a boundary whose preceding tool
results have settled. The boundary owns the necessary native message copy; size
estimation and summary projection borrow those messages. Retry progress reads
the text position and completed-call count without cloning history. Partial text
after that boundary is retracted before retrying, and cancellation interrupts
backoff and model reads.

Compaction projects repeated observations for summary input while leaving native
messages and persisted receipts intact. A partial checkpoint is reused only when
its processed prefix matches its recorded hash. Completed compaction replaces the
working context with a continuation checkpoint and keeps calls/results together.
The model receives business identities and outcomes; resource versions and other
consistency controls remain with the host.

Summary calls deliver their reasoning and text as ordered
`ContextCompactionDelta` events, batching at 40 ms or 4096 bytes and flushing
before a content-kind switch, completion or interruption. Their usage retains
the `Compaction` purpose. This output is persisted and displayed alongside each
agent's activity without entering its ordinary reply text. The completed summary
still uses the existing `ContextCompacted` event and checkpoint authority.

The summary prompt lives in [context/summary.md](src/context/summary.md), is loaded
as raw text with `include_str!`, and supplies no Markdown parser or prompt engine.

Focused validation from the repository root:

```sh
cargo test --locked -p yss-harness-rig --lib --offline
cargo clippy --locked -p yss-harness-rig --lib --tests --no-deps --offline -- -D warnings
cargo fmt -p yss-harness-rig -- --check
```

These tests use scripted providers and local HTTP fixtures. Live provider access
and desktop interaction require their own acceptance evidence.
