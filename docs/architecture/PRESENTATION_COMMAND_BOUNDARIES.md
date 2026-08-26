# Presentation 与 Command 边界

本文档定义 Tauri transport、Application 与 domain 的目标方向，并承担
`presentation-command` 精确债务组的清理责任。当前实现地图仍以
[ARCHITECTURE.md](./ARCHITECTURE.md) 为准。

## Dependency direction

```text
React UI
   │
Frontend services
   │
Tauri Commands / Events / Channels
   │
Application use cases
   │
Project / Graph / Execution / Database / SCI
```

Command 只执行：

1. parse/validate wire input；
2. 捕获所需 Tauri state 或 channel adapter；
3. 调用一个 Application public seam，或一个已批准的单-owner 原子 query；
4. 穷尽映射 typed result/error；
5. 交付 committed low-rate event 或 ordered run channel。

Command 不执行 filesystem I/O、Polars/DuckDB computation、project transaction、compiler/
runtime/backend construction、跨 owner 排序或重复 currentness validation。允许的直接 symbol
必须由 exact file + owner + canonical target capability 声明；layer prefix、glob 和 barrel
不能扩大权限。

Transport 独占 serializable DTO、`CommandError { code, details, incidentId }`、Tauri event 与
channel wire。Project、Graph、Execution、Database 和 SCI 不导入 command/event/schema/error
transport owner。Rust 不提供用户可见文案；React 根据 stable code 与 safe details 本地化并
选择 Alert、inline feedback 或 MessageDialog。

## 迁移规则

workflow 移入 Application 后，在同一 compiling slice 把 command 唯一切到新 seam，并删除
command 内旧 workflow、重复 mapper/test 和对应精确债务。事件转换按 Domain fact →
Application fact → Transport DTO 单向完成，不建立第二 listener、第二 channel 或双 emit。

完成条件：`debt/presentation_command.rs` 为空，Commands 只保留批准的 Application/Transport
capability，composition root 只负责构造与注册，production architecture audit、CommandError/
wire focused tests 和 `pnpm rust:check` 全部通过。
