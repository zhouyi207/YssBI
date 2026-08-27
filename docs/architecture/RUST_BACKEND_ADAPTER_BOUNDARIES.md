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

## Staged activation debt

`julia/bayes_worker_adapter/` 已实现最终 SCI `BayesWorkerPort`，但保持 production-unreachable。
它没有 `lib.rs`、Application、Commands 或 Project constructor caller；当前
`sci/backends/julia/**` 仍是唯一 active Bayes route。该 staged adapter 的激活与旧 pipeline
删除由 Execution Task 8 在同一 compiling slice 完成，不能提前增加 converter、forwarder、
fallback 或第二条 composition route。

`backend_adapters/execution/scientific.rs` 已实现 Execution-owned `ScientificBackend`，并仅在
该 exact adapter owner 内穷尽映射 Execution settings、request、control、result 与 SCI
public API/error。它保持 production-unreachable；当前 node runtime、commands 与 Application
仍直接调用 SCI，且是唯一 active scientific route。该 staged adapter 的构造注入、旧 direct-SCI
route 删除和 capability 收敛同样由 Execution Task 8 在同一 compiling slice 完成。

完成条件：`debt/backend_adapter.rs` 为空，且 production architecture audit、相关 focused
tests、`pnpm rust:check` 与 `git diff --check` 全部通过。
