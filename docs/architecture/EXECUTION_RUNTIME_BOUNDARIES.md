# Execution Runtime 边界

本文档定义 Execution Runtime 的目标 seam，并承担 `execution-runtime` 精确债务组的清理
责任。当前实现地图仍以 [ARCHITECTURE.md](./ARCHITECTURE.md) 为准。

## Execution owns

- immutable execution request、plan-local handle 与 validated parameters；
- scheduler、run identity、cancellation、deadline 和 bounded output；
- scientific、relational 与 resource provider ports；
- result candidate、effect intent 与 terminal finalization protocol；
- closed Execution-facing errors，不包含 backend prose。

Execution 不拥有 project authority、graph document、catalog/registry、UI action、Tauri DTO 或
concrete Polars/DuckDB/SCI/Julia type。

## Data flow and code dependency

```text
Graph compiler ──> immutable neutral plan contract
                                  │
Application ──> Execution Runtime │
                                  ▼
                      consumer-owned backend ports
                       │       │       │       │
                    Polars   DuckDB  RustSCI  Julia
```

Graph 产生 immutable plan 的业务数据流，但 Graph 与 Execution 的 Rust module 通过中立
contract 对接，不互相读取 implementation。Application 捕获 project/session basis，把显式
request 和可验证 capability 交给 Execution；Execution 不持有 `ProjectState` 或在运行中查询
Graph。

Concrete engine 只由 composition root 注入的 backend adapter 调用。Kernel 只消费
plan-local parameter、grant-scoped resource 和 typed backend result。取消、deadline、output
sequence 与 terminal result 必须属于同一 run identity。

## 迁移规则

抽取 port 或 runtime owner 时，先通过 public seam 的 focused test 固定 observable contract，
再在同一 compiling slice 切换唯一 production composition，删除旧 registry/provider/kernel
route、旧 constructor 和对应精确债务。不得设置全局 registry、隐式 default backend 或第二条
执行链。

完成条件：`debt/execution_runtime.rs` 为空，Execution production source 只依赖自己的模块与
Pure Leaf contract，所有 concrete engine origin 均位于批准的 adapter owner，production
architecture audit 与 execution focused tests 全部通过。
