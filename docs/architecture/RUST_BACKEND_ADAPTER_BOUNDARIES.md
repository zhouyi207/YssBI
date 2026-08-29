# Rust Backend Adapter 边界

本文档定义 Backend Adapter 的目标依赖方向，并承担
`rust-backend-adapter` 精确债务组的清理责任。当前实现地图仍以
[ARCHITECTURE.md](./ARCHITECTURE.md) 为准。

## 目标结构

```text
Application / Graph / Project / Execution
                  │
          consumer-owned ports
                  │
        ┌─────────┼──────────┐
        ▼         ▼          ▼
   Database     Rust SCI    Julia host
        │
   DuckDB / Polars / SQLx
```

- 核心 module 只依赖自己拥有的 port 或顶层 Pure Leaf contract。
- concrete backend 位于 `backend_adapters/`、`julia/` 或明确分类的
  platform adapter；composition root 负责构造并注入。
- SCI core/API 不导入 Graph、Project、Execution、Commands 或 Tauri。
- Database Core 不导入 Application、Project、Commands 或 Tauri runtime。
- Polars、DuckDB、SQLx、Julia process、Windows API 和 Tauri runtime 只能出现在
  policy 明确批准的 adapter owner；不得通过 barrel 暴露给核心层。

## Contract ownership

- persisted scalar/value shape 属于 `data_contract/`。
- database declaration/engine identity 属于 `database_contract/`。
- neutral tabular snapshot 属于 `tabular/contract.rs`。
- SCI-facing request/result/error 属于 SCI；Execution-facing scientific
  request/result/error/control 与 operation port 直接属于 Execution。
- backend 负责在上述 contract 与 concrete engine type 之间穷尽转换。

Port error 使用 closed typed enum。原始 driver、process、SQL 或 numeric-library 错误只进入
sanitized `tracing`，不能成为 IPC code、details 或成功 DTO 的一部分。长 I/O、worker wait
和数据库操作不得持有全局 project lock。

## 迁移规则

每次移动一个 canonical owner 时，在同一 compiling slice 中切换全部 production caller，
删除旧 declaration、旧 route、重复 test 和对应精确债务。任何时刻只有一条 production
route；不建立 forwarder、双注册或按运行时条件选择旧新实现的分支。

## Active composition

`julia/bayes_worker_adapter/` 是唯一 production Bayes worker adapter，实现最终 SCI
`BayesWorkerPort`。`lib.rs` 将它注入 `Application::BayesInferenceService`；Application 从
Project/Database coherent snapshot 生成 typed `StatisticalInput`，worker 负责 task handle、
generation、artifact ownership 与 Julia exchange。旧 `sci/backends/julia/**` route 已删除。

`backend_adapters/execution/scientific.rs` 已实现 Execution-owned `ScientificBackend`，并仅在
该 exact adapter owner 内穷尽映射 Execution settings、request、control、result 与 SCI
public API/error。`lib.rs` 将它注入 `ExecutionRuntimeState`；Application statistics 与
Execution runtime 不再直接调用 concrete SCI implementation。
当前 SCI operations 是同步 API，adapter 只在 method admission 时将 Execution control 映射为
Task 2 SCI control 并检查 cancellation/deadline；这不是 mid-computation cooperative
cancellation 承诺；不增加 polling shim 或伪异步路径。

`backend_adapters/execution/bayes_artifacts.rs` 是唯一 Polars/IPC Bayes artifact reader，
实现 Application-owned `BayesArtifactReader`。Application 不再直接持有 Polars artifact
parsing implementation，SCI worker result 只通过 typed reader 进入 presentation 查询。

完成条件：`debt/backend_adapter.rs` 为空，production adapters 已在唯一 composition root
激活；production architecture audit、`pnpm rust:check` 与 `git diff --check` 已通过。
