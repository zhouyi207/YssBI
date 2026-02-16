┌──────────────────────────┐
│ ProjectIOStore           │  ← load / save / export
└───────────┬──────────────┘
            │ hydrate
┌───────────▼──────────────┐
│ VariableStore             │  ← 低频
├──────────────────────────┤
│ DatabaseStore              │  ← 低频
├──────────────────────────┤
│ GraphMetaStore             │  ← graph 列表 / schema
├──────────────────────────┤
│ GraphDataStore             │  ← ★编辑器核心数据（可 undo）
├──────────────────────────┤
│ GraphRuntimeStore          │  ← ★UI / 交互 / 相机（不可 undo）
└──────────────────────────┘
