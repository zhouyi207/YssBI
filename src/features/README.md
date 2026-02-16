# Features 目录结构

本目录采用三层架构组织代码，确保清晰的依赖关系和职责划分。

## 层次说明

### Core 层（核心层）

**职责：** 提供基础设施和核心数据模型

**特点：**
- 无依赖或最小依赖
- 可以被其他层依赖
- 包含最基础的功能


### Domain 层（领域层）

**职责：** 实现业务领域逻辑

**特点：**
- 依赖 Core 层
- 不依赖 Application 层
- 包含领域特定的业务逻辑


### Application 层（应用层）

**职责：** 协调各个领域，提供完整的应用功能

**特点：**
- 依赖 Core 和 Domain 层
- 组合多个 domain feature
- 提供用户可见的功能


## 依赖规则

### ✅ 允许的依赖

- Application → Domain
- Application → Core
- Domain → Core

### ❌ 禁止的依赖

- Core → Domain
- Core → Application
- Domain → Application
- 同层之间的循环依赖

