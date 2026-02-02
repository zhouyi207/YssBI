# Print 节点日志集成完成

## 功能概述

Print 节点的输出现在会同时发送到：
1. 执行日志（原有功能）
2. 日志窗口（新增功能）

## 实现内容

### 修改文件
`src-tauri/src/executor/node/catalog/debug.rs`

### 修改内容

#### 修改前
```rust
print_node.set_flow_processor(Box::new(|ctx, node| {
    if !node.inputs.is_empty() {
        let val = ctx.get_pin_value(&node.inputs[0].id);
        ctx.log(format!("[Print] {}", val));
    } else {
        ctx.log("[Print] No input value".to_string());
    }
    Ok("Out".into())
}));
```

#### 修改后
```rust
print_node.set_flow_processor(Box::new(|ctx, node| {
    if !node.inputs.is_empty() {
        let val = ctx.get_pin_value(&node.inputs[0].id);
        let message = format!("{}", val);
        
        // 发送到执行日志
        ctx.log(format!("[Print] {}", message));
        
        // 发送到日志窗口（应用程序日志）
        crate::log_app!(
            crate::logging::LogLevel::Info,
            message,
            "Print"
        );
    } else {
        ctx.log("[Print] No input value".to_string());
        
        crate::log_app!(
            crate::logging::LogLevel::Warn,
            "No input value",
            "Print"
        );
    }
    Ok("Out".into())
}));
```

## 日志类型说明

### 执行日志
- 通过 `ctx.log()` 发送
- 包含 `[Print]` 前缀
- 用于执行追踪和调试

### 应用程序日志
- 通过 `log_app!` 宏发送
- 日志类型: `Application`
- 日志级别: 
  - `Info`: 正常打印
  - `Warn`: 无输入值
- 来源: `"Print"`
- 直接显示打印内容（不含前缀）

## 使用示例

### 场景 1: 打印字符串

**图配置**
```
Event On Run → Print("Hello World") → Plot
```

**日志窗口显示**
```
12:34:56.789  INFO   [APP]  [Print]  Hello World
```

### 场景 2: 打印变量

**图配置**
```
Event On Run → Get Variable(count=42) → To String → Print → Plot
```

**日志窗口显示**
```
12:34:56.789  INFO   [APP]  [Print]  42
```

### 场景 3: 打印多个值

**图配置**
```
Event On Run → Sequence
  ├─ Then 0 → Print("Start")
  ├─ Then 1 → Print("Processing")
  └─ Then 2 → Print("Done")
```

**日志窗口显示**
```
12:34:56.789  INFO   [APP]  [Print]  Start
12:34:56.790  INFO   [APP]  [Print]  Processing
12:34:56.791  INFO   [APP]  [Print]  Done
```

### 场景 4: 无输入值（警告）

**图配置**
```
Event On Run → Print (未连接输入)
```

**日志窗口显示**
```
12:34:56.789  WARN   [APP]  [Print]  No input value
```

## 日志窗口过滤

### 查看所有 Print 输出
1. 打开日志窗口
2. 在搜索框输入 "Print"
3. 或者在过滤器中只选择 "应用" 类型

### 查看特定内容
1. 在搜索框输入关键词
2. 例如搜索 "Error" 查看所有包含错误的打印

## 与其他日志的区别

### Print 节点日志
- 类型: Application
- 级别: Info/Warn
- 来源: Print
- 用途: 用户主动打印的内容

### 执行日志
- 类型: Execution
- 级别: Info/Debug
- 来源: 节点名称
- 用途: 执行流程追踪

### 系统日志
- 类型: System
- 级别: Info/Warn/Error
- 来源: 模块名称
- 用途: 系统级事件

## 优势

### 1. 统一的日志查看
- 不需要单独的控制台窗口
- 所有输出集中在日志窗口
- 支持过滤和搜索

### 2. 持久化存储
- Print 输出自动保存到日志文件
- 可以回溯查看历史输出
- 支持导出和分析

### 3. 更好的调试体验
- 可以同时查看 Print 输出和执行日志
- 时间戳精确到毫秒
- 支持按级别过滤

### 4. 灵活的过滤
- 按日志类型过滤
- 按日志级别过滤
- 按关键词搜索
- 按来源过滤

## 后续优化建议

### 1. 添加 Print 节点变体
- **Print Debug**: 发送到 Debug 级别
- **Print Warn**: 发送到 Warn 级别
- **Print Error**: 发送到 Error 级别

### 2. 支持格式化打印
- 类似 printf 的格式化字符串
- 支持多个输入参数
- 自动格式化对象

### 3. 条件打印
- 添加 Enable 输入 pin
- 只在条件为 true 时打印
- 减少不必要的日志

### 4. 打印到文件
- 添加 File 输入 pin
- 支持打印到指定文件
- 支持追加或覆盖模式

## 测试建议

### 功能测试
1. 创建包含 Print 节点的图
2. 执行图
3. 打开日志窗口
4. 确认 Print 输出显示在日志窗口中
5. 确认日志类型为 "应用"
6. 确认来源为 "Print"

### 过滤测试
1. 执行包含多个 Print 的图
2. 在搜索框输入关键词
3. 确认只显示匹配的日志
4. 切换日志类型过滤
5. 确认过滤正常工作

### 持久化测试
1. 执行包含 Print 的图
2. 关闭应用
3. 重新打开应用
4. 打开日志窗口
5. 确认历史 Print 输出仍然存在

### 性能测试
1. 创建包含大量 Print 的图（100+）
2. 执行图
3. 确认日志窗口响应流畅
4. 确认内存占用正常

## 相关文件

- `src-tauri/src/executor/node/catalog/debug.rs` - Print 节点实现
- `src-tauri/src/logging.rs` - 日志管理器
- `src/components/LogView/LogWindow.tsx` - 日志窗口 UI
- `src/store/logStore.ts` - 日志状态管理

## 完成时间
2026-02-02
