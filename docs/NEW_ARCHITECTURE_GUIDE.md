# 新架构指南

## 概述

本文档描述了按照 `fix_node.md` 和 `fix_pin.md` 要求重构后的新架构。

## 核心原则

1. **Node 不持有 Pin** - 节点只是定义模板
2. **Pin 不属于 Node** - Pin 由 Graph 统一管理
3. **Graph 是唯一真实来源** - 所有运行时状态在 Graph 中
4. **语义角色访问** - 通过 PinRole 而非 index/name 访问 Pin
5. **定义与实例分离** - NodeDefinition（静态）vs NodeInstance（运行时）

## 核心数据结构

### PinRole - 语义角色

```rust
pub enum PinRole {
    // 控制流
    ExecIn, ExecOut, ExecTrue, ExecFalse,
    
    // 数据
    Condition, Input, Output, Result,
    
    // 动态组
    Operands, Steps, Cases, Elements,
    
    // 自定义
    Custom(String),
}
```

### NodeDefinition - 节点定义（静态）

包含节点类型、Pin 定义、处理器等，不包含运行时状态。

### NodeInstance - 节点实例（运行时）

仅包含 ID 和定义引用，不持有 Pin。

### PinInstance - Pin 实例（运行时）

包含状态和值，不包含连接信息。

### Graph - 运行时世界

管理所有节点、Pin、连接和状态。

## 使用示例

详见 `src-tauri/src/executor/node/examples.rs`

