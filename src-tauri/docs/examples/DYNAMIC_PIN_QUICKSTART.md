# 动态 Pin 快速开始

## 最简单的调用方式

### 前端代码（3 步）

```typescript
import { invoke } from '@tauri-apps/api/core';

// 1. 添加输入 Pin
const result = await invoke('add_node_dynamic_pin', {
  subgraphId: 'event_1',
  nodeId: 'node_123',
  pinType: 'data',      // 'data' 或 'exec'
  direction: 'input'    // 'input' 或 'output'
});

console.log(result);
// {
//   pinId: "550e8400-...",
//   name: "Input 3",
//   type: "Data",
//   direction: "Input"
// }

// 2. 移除 Pin
await invoke('remove_node_dynamic_pin', {
  subgraphId: 'event_1',
  nodeId: 'node_123',
  pinId: result.pinId
});

// 3. 检查能力（可选）
const capability = await invoke('get_node_dynamic_constraints', {
  subgraphId: 'event_1',
  nodeId: 'node_123'
});

console.log(capability);
// {
//   canAddPins: true,
//   dynamicConfigs: [...]
// }
```

## React 组件示例（最简版）

```tsx
import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';

function DynamicNodeControls({ subgraphId, nodeId }) {
  const [loading, setLoading] = useState(false);

  const handleAddInput = async () => {
    setLoading(true);
    try {
      const result = await invoke('add_node_dynamic_pin', {
        subgraphId,
        nodeId,
        pinType: 'data',
        direction: 'input'
      });
      console.log('Added:', result);
      // 刷新节点显示
    } catch (error) {
      alert('Failed: ' + error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <button onClick={handleAddInput} disabled={loading}>
      {loading ? 'Adding...' : '+ Add Input'}
    </button>
  );
}
```

## 参数说明

### add_node_dynamic_pin

| 参数 | 类型 | 值 | 说明 |
|------|------|-----|------|
| subgraphId | string | "event_1" | 子图 ID |
| nodeId | string | "node_123" | 节点 ID |
| pinType | string | "data" \| "exec" | Pin 类型 |
| direction | string | "input" \| "output" | Pin 方向 |

### remove_node_dynamic_pin

| 参数 | 类型 | 值 | 说明 |
|------|------|-----|------|
| subgraphId | string | "event_1" | 子图 ID |
| nodeId | string | "node_123" | 节点 ID |
| pinId | string | "550e8400-..." | 要移除的 Pin ID |

## 常见问题

### Q: 如何知道节点支持动态 Pin？

A: 调用 `get_node_dynamic_constraints`，检查 `canAddPins` 字段。

```typescript
const capability = await invoke('get_node_dynamic_constraints', {
  subgraphId,
  nodeId
});

if (capability.canAddPins) {
  // 显示添加按钮
}
```

### Q: 如何知道还能添加多少个 Pin？

A: 检查 `dynamicConfigs` 中的 `maxCount`。

```typescript
const config = capability.dynamicConfigs[0];
const currentCount = node.inputs.length;
const canAdd = currentCount < config.maxCount;
```

### Q: 添加失败怎么办？

A: 捕获错误并显示提示。

```typescript
try {
  await invoke('add_node_dynamic_pin', { ... });
} catch (error) {
  if (error.includes('Cannot add more pins')) {
    alert('已达到最大输入数量');
  } else {
    alert('添加失败: ' + error);
  }
}
```

### Q: 如何刷新节点显示？

A: 监听 `project-event` 事件或重新获取节点数据。

```typescript
import { listen } from '@tauri-apps/api/event';

await listen('project-event', (event) => {
  if (event.payload.eventType === 'NodesUpdated') {
    // 更新节点显示
    refreshNodes(event.payload.data.nodes);
  }
});
```

## 完整示例

查看详细文档：
- [前端集成指南](./DYNAMIC_PIN_FRONTEND_GUIDE.md) - 完整的 TypeScript 实现
- [动态 Pin 总结](./DYNAMIC_PIN_SUMMARY.md) - 后端实现说明
