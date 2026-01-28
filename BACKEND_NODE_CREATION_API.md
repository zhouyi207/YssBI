# 后端创建节点 API 使用指南

## 概述

后端创建节点功能已实现，允许从后端统一管理节点的创建、验证和ID生成。

## API 接口

### 1. 创建节点 (create_node)

**后端 Rust 命令：**
```rust
#[tauri::command]
fn create_node(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node: SerializedNode,
) -> Result<SerializedNode, String>
```

**前端 TypeScript 方法：**
```typescript
static async createNode(subgraphId: string, node: any): Promise<any>
```

**功能：**
- 在指定子图中创建一个新节点
- 自动验证节点ID的唯一性
- 返回创建后的节点数据
- 自动触发 `NodesUpdated` 事件通知所有窗口

**使用示例：**

```typescript
import { ProjectService } from '../services/projectService';
import { v4 as uuidv4 } from 'uuid';

// 示例：创建一个新节点
async function handleNodeDrop(nodeType: string, position: { x: number, y: number }) {
  try {
    // 1. 构造节点数据
    const newNode = {
      id: uuidv4(), // 生成唯一ID
      type: nodeType,
      title: `${nodeType} Node`,
      position: position,
      isInternal: false,
      inputs: [],
      outputs: [],
      // 可选字段...
    };

    // 2. 调用后端创建节点
    const createdNode = await ProjectService.createNode(
      'event-main', // 子图ID
      newNode
    );

    console.log('节点创建成功:', createdNode);
    
    // 3. 节点已自动添加到后端状态，并触发了事件
    // 前端监听 NodesUpdated 事件即可自动更新UI
    
  } catch (error) {
    console.error('创建节点失败:', error);
    // 处理错误，比如ID重复
  }
}
```

---

### 2. 删除节点 (delete_node)

**后端 Rust 命令：**
```rust
#[tauri::command]
fn delete_node(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
) -> Result<(), String>
```

**前端 TypeScript 方法：**
```typescript
static async deleteNode(subgraphId: string, nodeId: string): Promise<void>
```

**功能：**
- 从指定子图中删除节点
- 自动验证节点是否存在
- 自动触发 `NodesUpdated` 事件通知所有窗口

**使用示例：**

```typescript
// 示例：删除节点
async function handleNodeDelete(nodeId: string) {
  try {
    await ProjectService.deleteNode(
      'event-main', // 子图ID
      nodeId
    );

    console.log('节点删除成功');
    
  } catch (error) {
    console.error('删除节点失败:', error);
  }
}
```

---

## 完整工作流示例

### 场景：拖拽创建节点

```typescript
import { ProjectService } from '../services/projectService';
import { v4 as uuidv4 } from 'uuid';

interface NodePosition {
  x: number;
  y: number;
}

/**
 * 用户从节点面板拖拽节点到画布
 */
async function onNodeDragDrop(
  nodeType: string,
  position: NodePosition,
  subgraphId: string
) {
  try {
    // 1. 生成新节点数据
    const newNode = {
      id: uuidv4(),
      type: nodeType,
      title: getDefaultTitle(nodeType),
      position: position,
      isInternal: false,
      inputs: getDefaultInputs(nodeType),
      outputs: getDefaultOutputs(nodeType),
    };

    // 2. 后端创建节点
    const createdNode = await ProjectService.createNode(subgraphId, newNode);

    // 3. 可选：立即在UI上高亮新节点
    highlightNode(createdNode.id);

    return createdNode;
    
  } catch (error) {
    console.error('节点创建失败:', error);
    showErrorNotification('无法创建节点: ' + error);
    throw error;
  }
}

/**
 * 用户删除节点
 */
async function onNodeDelete(nodeId: string, subgraphId: string) {
  try {
    // 确认删除
    const confirmed = await showConfirmDialog('确定要删除这个节点吗？');
    if (!confirmed) return;

    // 后端删除节点
    await ProjectService.deleteNode(subgraphId, nodeId);

    showSuccessNotification('节点已删除');
    
  } catch (error) {
    console.error('节点删除失败:', error);
    showErrorNotification('无法删除节点: ' + error);
    throw error;
  }
}

// 辅助函数示例
function getDefaultTitle(nodeType: string): string {
  const titles: Record<string, string> = {
    'Print': '打印节点',
    'Add': '加法节点',
    'Subtract': '减法节点',
    // ...
  };
  return titles[nodeType] || nodeType;
}

function getDefaultInputs(nodeType: string) {
  // 根据节点类型返回默认输入引脚
  return [];
}

function getDefaultOutputs(nodeType: string) {
  // 根据节点类型返回默认输出引脚
  return [];
}
```

---

## 事件监听

后端创建/删除节点会自动触发 `NodesUpdated` 事件，前端可以监听这个事件来更新UI：

```typescript
import { listen } from '@tauri-apps/api/event';

// 监听节点更新事件
const unlisten = await listen('project-event', (event) => {
  const { type, payload } = event.payload;
  
  if (type === 'NodesUpdated') {
    const { subgraph_id, nodes } = payload;
    console.log(`子图 ${subgraph_id} 的节点已更新:`, nodes);
    
    // 更新UI
    updateNodesInCanvas(subgraph_id, nodes);
  }
});

// 清理监听器
// unlisten();
```

---

## 错误处理

### 常见错误

1. **节点ID重复**
   ```
   Error: "Node with id 'xxx' already exists in subgraph 'yyy'"
   ```
   **解决方案：** 确保生成唯一的UUID

2. **子图不存在**
   ```
   Error: "Subgraph 'xxx' not found"
   ```
   **解决方案：** 检查子图ID是否正确

3. **节点不存在（删除时）**
   ```
   Error: "Node with id 'xxx' not found in subgraph 'yyy'"
   ```
   **解决方案：** 检查节点是否已被删除

### 错误处理最佳实践

```typescript
async function createNodeSafely(subgraphId: string, node: any) {
  try {
    return await ProjectService.createNode(subgraphId, node);
  } catch (error) {
    // 记录错误
    console.error('[NodeCreation] Failed:', error);
    
    // 根据错误类型提供友好提示
    if (error.toString().includes('already exists')) {
      showWarning('节点ID冲突，请重试');
    } else if (error.toString().includes('not found')) {
      showError('子图不存在，请刷新页面');
    } else {
      showError('创建节点失败，请稍后重试');
    }
    
    throw error;
  }
}
```

---

## 性能考虑

### 批量操作

如果需要**批量创建或删除节点**，建议使用 `setNodes` 方法而不是多次调用 `createNode`/`deleteNode`：

```typescript
// ❌ 不推荐：多次调用
for (const node of newNodes) {
  await ProjectService.createNode(subgraphId, node);
}

// ✅ 推荐：批量操作
const currentNodes = await ProjectService.getNodes(subgraphId);
const updatedNodes = [...currentNodes, ...newNodes];
await ProjectService.setNodes(subgraphId, updatedNodes);
```

---

## 测试

### 单元测试示例

```typescript
import { describe, it, expect, vi } from 'vitest';
import { ProjectService } from '../services/projectService';

describe('ProjectService - Node Operations', () => {
  it('should create a node successfully', async () => {
    const node = {
      id: 'test-node-1',
      type: 'Print',
      title: 'Test Node',
      position: { x: 100, y: 100 },
      isInternal: false,
      inputs: [],
      outputs: [],
    };

    const result = await ProjectService.createNode('event-main', node);
    
    expect(result.id).toBe('test-node-1');
    expect(result.type).toBe('Print');
  });

  it('should delete a node successfully', async () => {
    await expect(
      ProjectService.deleteNode('event-main', 'test-node-1')
    ).resolves.not.toThrow();
  });

  it('should throw error when creating duplicate node', async () => {
    const node = {
      id: 'duplicate-id',
      type: 'Print',
      // ...
    };

    await ProjectService.createNode('event-main', node);
    
    await expect(
      ProjectService.createNode('event-main', node)
    ).rejects.toThrow('already exists');
  });
});
```

---

## 总结

✅ **已实现功能：**
- 后端创建单个节点 (create_node)
- 后端删除单个节点 (delete_node)
- 自动ID唯一性验证
- 自动事件通知

✅ **优势：**
- 统一的数据验证
- 单一真相来源（后端状态）
- 自动事件同步到所有窗口

⚠️ **注意事项：**
- 批量操作时建议使用 `setNodes`
- 需要生成唯一UUID（推荐使用 uuid 库）
- 记得监听 `NodesUpdated` 事件更新UI
