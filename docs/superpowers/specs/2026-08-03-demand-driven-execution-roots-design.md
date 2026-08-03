# Demand-Driven Execution Roots Design

## Status

Approved design for completing the first production consumption of `EvaluationPolicy` and requested graph outputs.

## Constraints

- Work directly on `shadcn`; do not create a worktree, branch, commit, or tag.
- Preserve unrelated dirty work.
- Rust remains authoritative for graph protocol, semantic analysis, compilation, resources, and execution.
- Do not expose or persist compiler-local `ValueRef`, `OperationIndex`, or `valueIndex` as request/result identity.
- Requested outputs do not change `CompilationBasis`.
- Full graph analysis and diagnostics remain available; only executable plans are demand-specialized.
- Preserve existing Branch, Loop, Call, effect ordering, cancellation, finalization, and resource cleanup behavior.
- Use focused RED-GREEN tests and update `TODO.md` after every independently reviewed task.

## Problem

Node protocols declare:

- `EvaluationPolicy::DemandDriven`;
- `EvaluationPolicy::EagerWhenRegionEntered`;
- `Purity`;
- `CachePolicy`;
- effect semantics.

The compiler currently lowers the complete graph into one `ExecutionPlan`, and the runtime executes every operation in `root_region`. `EvaluationPolicy` and `CachePolicy` do not affect compilation or scheduling. Consequently, disconnected and unrequested pure nodes execute, their resources are preflighted/acquired, and relational planning cannot prune unused outputs.

The canonical execution request also lacks requested outputs. Existing `valueReady.valueIndex` is compiler-local and cannot be used as stable frontend pin identity.

## 1. Stable requested-output identity

Add a Rust identity equivalent to:

```rust
pub struct GraphOutputRef {
    pub graph_path: GraphResourcePath,
    pub port: PortAddress,
}
```

The IPC DTO uses the existing `PortAddressDto` shape:

```ts
type GraphOutputRefDto = {
  graphPath: string;
  port: PortAddressDto;
};
```

This identity supports declared ports and dynamic port instances through stable `NodeId`, `PortKey`, template key, and `PortInstanceId` values.

The first slice accepts only outputs whose `graphPath` equals the top-level graph being executed. Requested ports must resolve in the authoritative compilation snapshot and must be materialized, non-orphan, data output ports. Nested function output preview is rejected with a structured error because function activations are not uniquely identified by graph path and port alone.

`ValueRef`, `OperationIndex`, `PlanResult.name`, frontend pin display IDs, and run `sourceId` are not accepted request identities.

## 2. Execution demand

Add an explicit demand model:

```rust
pub enum ExecutionDemand {
    Default,
    Outputs {
        outputs: Box<[GraphOutputRef]>,
        include_default_results: bool,
    },
}
```

The IPC representation is a strict tagged union:

```ts
type ExecutionDemandDto =
  | { type: 'default' }
  | {
      type: 'outputs';
      outputs: GraphOutputRefDto[];
      includeDefaultResults: boolean;
    };
```

Semantics:

- `Default` uses existing lowerer-declared `FragmentMetadata.results` as normal results.
- `Outputs` adds requested graph outputs as results.
- `include_default_results` controls whether lowerer-declared defaults are unioned with requested outputs.
- An empty output list with `include_default_results == false` still runs eager/effect roots and produces no ordinary output result.
- Caller order and duplicates do not affect plan identity or behavior.

Update `ProjectState::execute_graph`, canonical Tauri execution command, frontend service, and all call sites directly. This 0.x project does not keep an old overload or compatibility request path.

## 3. Full compiler product and demand variants

Keep `CompilationBasis` unchanged. It identifies graph revision, registry fingerprint, and resource versions, not execution selection.

A basis-level compiler product retains:

- full analysis and diagnostics;
- validated semantic graph/interface facts;
- a lowering intermediate before final relational grouping and dense operation indexing;
- stable `GraphOutputRef`/`PortAddress` to internal value lookup;
- lowerer-declared default results;
- operation evaluation/purity/effect semantics;
- operation-owned resource requirements.

Demand-specific plans derive from this full product. Their normalized `DemandKey` contains:

```text
mode
include_default_results
sorted and deduplicated GraphOutputRef set
```

The compile coordinator continues to publish one current basis-level product. Demand variants must not overwrite current analysis/projection or appear as graph authority changes.

A bounded variant cache may live inside or adjacent to the basis-level product. Cache eviction changes performance only. Graph revision, registry fingerprint, resource-version, project-session, or authority changes invalidate all variants through the existing basis/product replacement.

Run observability must distinguish variants with a deterministic demand/selection digest while preserving the full compile ID as the basis product identity.

## 4. Compiler pruning stage

Demand closure runs after full semantic validation and node lowering facts exist, but before:

- final relational island grouping;
- final dense `OperationIndex` allocation;
- final structured region projection;
- final resource aggregation;
- `ExecutionPlan::validate`.

Do not prune semantic analysis or diagnostics.

The lowering intermediate must retain operation semantics from `NodeProtocol.execution`:

```text
evaluation policy
purity
effect semantics
operation inputs and outputs
operation-owned resources
structured role/bindings
```

`CachePolicy` may remain recorded for future work, but this slice does not implement cross-run or activation memoization.

## 5. Root and dependency closure

Roots are established per structured region:

1. explicitly requested graph outputs;
2. default `FragmentMetadata.results` when selected by demand mode;
3. every `EagerWhenRegionEntered` operation in an entered region;
4. effect predecessors required by retained effect ordering;
5. conditions, carried values, arguments, and result bindings required by retained structured regions.

For a demanded value:

- follow incoming value dependencies to their sources;
- retain its producing operation and demand all required operation inputs;
- retain structured producer bindings and their source values;
- continue to a fixed point.

A disconnected `DemandDriven + pure` operation that is not in a requested/default dependency closure is removed.

Do not infer evaluation from purity or from the presence of an effect edge. `EvaluationPolicy` is authoritative.

## 6. Structured region rules

### Sequence

Remove unretained demand-driven pure operations. Preserve retained operations and non-empty child regions. Reassign dense operation indices only after pruning.

### If

Retain an `If` when a requested/default result depends on it or either arm contains an eager/effect root.

A retained `If` keeps:

- condition closure;
- both then and else structural regions;
- both sources for each retained branch result binding;
- branch-local eager/effect operations in their original arm.

The compiler does not select an arm.

### Loop

Retain a `Loop` when a loop result is demanded or the body contains eager/effect roots.

A retained loop keeps:

- continue condition;
- initial source;
- body input;
- next source;
- result binding;
- required body closure.

Unrelated body-local pure operations may be removed. Existing iteration, carried value, limit, cancellation, and drain semantics remain unchanged.

### Call

A retained caller result keeps the Call region and required caller arguments. Calls required by control/effect semantics remain retained.

The first slice publishes and executes complete callee plans. Caller demand is not propagated into `FunctionPlanStore`, and callee ABI/result layout is not specialized.

## 7. Resource ownership

Resource requirements must be attributable to their owning intermediate operation or retained relational fragment.

After pruning:

- aggregate only retained requirements;
- preserve deterministic ordering and deduplication;
- retain all requirements needed by retained operations/subplans;
- do not validate or acquire resources used exclusively by pruned operations.

This is a correctness requirement: an unavailable resource belonging only to an unrequested pure node must not block the run.

## 8. Relational planning

Perform relational grouping after graph-level demand closure.

The relational planner receives only retained fragments and retained required outputs. It must preserve:

- requested graph outputs as island roots;
- outputs required by cross-island materialization bridges;
- bridge consumption/production semantics;
- owner operation outputs and compiled root cardinality/order;
- deterministic grouping independent of document insertion order.

Every derived relational plan must pass existing strict validation. This slice does not add Filter/Project lineage.

## 9. Plan results and canonical events

Extend result metadata so each requested graph result retains stable source identity:

```rust
pub struct PlanResult {
    pub output: GraphOutputRef,
    pub name: Box<str>,
    pub value: ValueRef,
}
```

Default result naming may remain for normal result consumers, but stable output identity is authoritative for pin preview.

Add a canonical event equivalent to:

```text
OutputReady {
    output: GraphOutputRefDto,
    sourceId: string
}
```

Frontend pin preview indexes by `(graphPath, PortAddress)`. It must not use `valueIndex` as identity. Existing compiler-local `valueReady` must not be extended into a public request contract; removal or deprecation within the canonical path may occur if all canonical consumers are migrated in this slice.

Stale project/run preview results cannot overwrite a newer preview result.

## 10. Validation and errors

Strictly validate requested output DTOs and authoritative resolution:

- top-level graph mismatch;
- missing node;
- missing declared port;
- missing/stale dynamic instance;
- orphan binding;
- input port;
- control/effect port;
- duplicate normalized identity;
- invalid graph/session/basis.

Duplicates are normalized, not treated as an error. All other invalid requests produce structured, stable errors before resource acquisition or kernel execution.

Every derived plan runs full `ExecutionPlan::validate`, including operation references, dependencies, structured bindings, results, resources, and relational roots.

## 11. Testing

Use RED-GREEN TDD.

Compiler tests cover:

- independent pure chains and single requested root;
- request order/duplicate determinism;
- default result inclusion modes;
- arbitrary valid data output not previously declared as a default result;
- invalid declared/dynamic/orphan/input/control/effect outputs;
- disconnected eager/effect roots;
- effect predecessor closure;
- If, Loop, and Call structured closure;
- pruned operation resources;
- relational retained roots/bridges/cardinality;
- derived plan validation.

Runtime/project production tests cover:

- unrequested pure kernel execution count is zero;
- eager/effect kernel executes exactly once as required;
- unavailable pruned resource is not validated/acquired;
- retained resources preserve existing preflight and RAII cleanup;
- Branch selected-arm, Loop carried values, Call frames, effect ordering, cancellation, finalization, and variable commits remain unchanged;
- default and multiple preview demands reuse one basis-level compile product;
- basis changes invalidate all variants;
- variants do not overwrite current analysis/projection;
- empty requested set executes eager/effects without ordinary results.

IPC/frontend tests cover:

- strict demand serde;
- declared and dynamic output identities;
- no compiler-local request identity;
- default run sends `Default`;
- preview sends one stable output;
- stable `OutputReady` updates the matching pin only;
- stale preview suppression;
- terminal channel drain and error precedence remain unchanged.

## 12. Explicit exclusions

This slice does not implement:

- nested function pin preview;
- demand-specialized callee plans;
- cross-run or activation value caching;
- user-persisted always-retain outputs;
- scheduler parallelism;
- run timeout/deadline policy;
- relational Filter/Project migration;
- full legacy execution-stack rewrite;
- automatic terminal-output inference;
- compile-time branch selection;
- changes to runtime recursion limit or structured-control ABI.

## 13. Verification

Run focused compiler, plan, runtime, project production, IPC, frontend service, preview, and structured-control regression tests. Run Rust filters serially with `CARGO_BUILD_JOBS=1`.

Required gates:

```sh
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

An independent final reviewer must verify stable identities, basis/demand separation, complete eager/effect/structured closure, resource pruning correctness, relational root integrity, and no structured-control regression.

After clean review and fresh controller verification, update `.superpowers/sdd/2026-08-03-demand-driven-execution-roots/progress.md` and `TODO.md` under `## node_architecture 进度`.
