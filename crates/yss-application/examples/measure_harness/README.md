# Harness task measurement

Run `pnpm measure:harness <configuration.json>` from the repository root. This is an
Application example that calls the configured model through the ordinary Harness,
Rig, SQLite and Application owners. It does not start or control a desktop window.

```json
{
  "settingsDir": "C:/Users/you/AppData/Roaming/com.zjy.yssbi/settings",
  "outputDir": "./measurement-01",
  "datasetCsv": "./synthetic-regression.csv",
  "options": { "mode": "write", "reasoningEffort": null },
  "maxDurationSeconds": 900,
  "tasks": [{ "label": "mind-edit", "promptPath": "./mind-edit.md" }]
}
```

Paths resolve against the configuration file. The output directory must not exist.
The default creates a new project inside that directory. `projectPath` can instead
select a prepared measurement project; tasks then modify that project through the
normal owners. `datasetCsv` optionally imports a CSV as `Synthetic regression`
before measurement starts. Task prompts are Markdown files and are copied into the
output. Tasks share their conversation unless `newConversation` is true.

`seedRegressionGraph: true` prepares a 205-node graph through the Application
owners before model measurement. It requires a new project and a CSV with numeric
`x1` through `x5` and `y`. The analysis branch has five X pins and a regression,
summary and heteroskedasticity diagnostic; 200 disconnected branches check scoped
execution. Setup is excluded from task metrics. The adjacent Markdown prompts
cover the roadmap's graph, atomic batch, database, Mind and report scenarios.

`injectDatabaseConflict: true` requires a new project and the synthetic CSV. After
the first successful Data worker row read, a separate invocation commits `321.25`
to that page's first stable row ID in `x4` through the same Application gateway.
The worker receives its original read result, so its next write must confront the
changed owner version. `concurrent-commit.json` preserves the external writer's
real receipt. This deterministic interleaving exercises the normal conflict and
worker-continuation paths without operating a desktop window.

The runner copies provider configuration into its isolated settings directory and
resolves credentials through the existing OS credential service. It clears only
the copied retired-credential list, so reading the measurement configuration cannot
delete the desktop's retired credentials. `model` optionally uses the ordinary
`{ providerId, modelId }` selection; otherwise the configured default is used.
The optional task duration requests ordinary Harness cancellation, then waits for
admitted operations to finish their ledger and commit handoff.

`runtime.rs` composes the real adapters and observes schemas at driver admission.
`measurement.rs` derives metrics from persisted events and ledger records; it does
not maintain a second business state. Each numbered task directory contains the
prompt, response, summary, public invocation details and provider usage reports.
The normal SQLite ledger remains in `db/` for inspection.

`model_calls.rs` observes the existing executor boundary without changing its
outcomes. `modelBusiness` and `model-calls.json` count model-facing business calls;
the ledger totals also contain Core's automatic baseline reads. Keep those two
counts separate. Agent outcomes include failed admissions and continuations, so
zero business-tool failures must not be reported as zero agent failures.

`controlCalls` records the Core event identities and start/end times for every
delegation, continuation and plan call, including rejected calls. It is separate
from business-call metrics and from accepted `AgentRun` / plan events. Its elapsed
time includes waiting for workers; do not add it to nested business times.

Metric definitions:

- Schema bytes are compact UTF-8 input-schema JSON for the tools offered to each
  agent admission. Descriptions and provider envelopes are excluded; this is not
  a cumulative network-request size.
- Result size counts the existing Contract public success/failure projection,
  both UTF-8 bytes and Unicode scalar characters. Failure envelopes and control
  tool results are excluded. Accepted control events are listed separately.
- Duplicate reads require equal public arguments and equal successful public
  output within the task. Same-argument retries require a preceding failed call;
  a retry with corrected arguments remains visible in invocation details. Use
  `modelBusiness` for this comparison: malformed arguments are not retained and
  have no argument hash, so their repeat count is unknown rather than inferred
  from the ledger's empty replay arguments.
- Token sums include reported samples only. Missing fields stay null with zero
  reporting calls; they are not estimated. Input already includes cached tokens,
  and output already includes reasoning tokens.
- Wall time covers the complete submitted task. Tool times use the authoritative
  ledger and include binding and scheduling; concurrent intervals can overlap.
  Their difference must not be called pure model inference or waiting time.

Compare runs only with identical input projects, prompts, model and configuration.
Check the actual artifacts and requested failure behavior before treating a
completed model turn as task acceptance. These measurements do not replace the
Assistant UI's manual timing, failure and history-replay acceptance.

The 2026-10-06 roadmap measurements used a 5,000-row CSV generated with Python's
`random.Random(20261005)`: for each row, draw five independent `gauss(0, 1)` values
for `x1` through `x5`, then draw `gauss(0, .2)` for the residual and set
`y = 1 + 2*x1 - x2 + .5*x3 + .3*x4 - .7*x5 + residual`. Write all six numeric
values with six decimal places, with header `x1,x2,x3,x4,x5,y`. Mind measurements
used the separate 200-row CSV; Mind does not query or modify that database.
