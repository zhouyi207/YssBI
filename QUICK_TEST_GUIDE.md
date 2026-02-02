# 快速测试指南 - Pin 用户值功能

## 问题
之前在 Print 节点的 Value pin 上设置值后，执行时输出仍然是 `null`。

## 修复内容
**关键修复**: 在 `src/components/Editor/Utils/io.ts` 的 `serializeSubGraph` 函数中添加了 `userValue` 字段的序列化。

之前序列化 pins 时遗漏了 `userValue`，导致即使前端 store 中有值，发送到后端执行时也丢失了。

## 测试步骤

1. **刷新页面或重启应用**
   - 确保使用最新的代码

2. **创建测试场景**
   - 打开或创建一个 Event
   - 添加一个 Print 节点
   - 将 Event 的 Exec 输出连接到 Print 的 In 输入

3. **设置 Pin 值**
   - 在 Print 节点的 "Value" pin 旁边应该有一个文本输入框
   - 点击输入框，输入 "Hello World"
   - 按 Enter 或点击其他地方保存

4. **检查控制台日志**
   打开浏览器开发者工具 (F12)，应该看到：
   ```
   [PinInput] Saving value: { subgraphId: "...", nodeId: "...", pinId: "...", value: "Hello World", pinType: "any" }
   [PinInput] Value saved successfully to backend
   [PinInput] Updated frontend store for input pin: ...
   [PinInput] Frontend store updated successfully
   ```

5. **执行 Event**
   - 点击执行按钮（播放图标）
   - 查看输出

6. **预期结果**
   - ✅ 应该输出: "Hello World"
   - ❌ 不应该输出: null

## 如果还是输出 null

请检查以下内容：

1. **确认代码已更新**
   - 检查 `src/components/Editor/Utils/io.ts` 文件
   - 确认 `serializeSubGraph` 函数中的 inputs 和 outputs 映射包含 `userValue: p.userValue`

2. **查看执行日志**
   - 在后端控制台查找执行日志文件（`src-tauri/logs/execution_*.json`）
   - 打开最新的日志文件
   - 搜索 Print 节点的 inputs
   - 确认 Value pin 有 `userValue` 字段

3. **检查前端发送的数据**
   - 在浏览器控制台，在执行前添加断点或日志
   - 查看 `ProjectService.executeProject` 发送的数据
   - 确认节点的 inputs 包含 `userValue`

## 调试命令

在浏览器控制台执行：
```javascript
// 查看当前 tab 的节点数据
const store = useNodeStore.getState();
const tabId = "your-event-id";
const nodes = store.tabs[tabId]?.nodes;
console.log("Nodes:", nodes);

// 查看特定节点的 pins
const printNode = nodes.find(n => n.type === "print");
console.log("Print node inputs:", printNode?.inputs);
```

## 成功标志

✅ 输入框显示并可以输入值
✅ 保存时控制台显示成功日志
✅ 执行时输出正确的值（不是 null）
✅ 保存项目后重新加载，值仍然存在
✅ 执行日志文件中包含 userValue 字段
