# 动态 Pin 前端集成指南

## 概述

本指南展示如何在前端调用动态 Pin 功能，包括完整的 TypeScript 代码示例。

## 后端 API

### 1. 获取节点的动态能力

```rust
#[tauri::command]
pub fn get_node_dynamic_constraints(
    subgraph_id: String,
    node_id: String,
) -> Result<serde_json::Value, String>
```

**返回示例**：
```json
{
  "canAddPins": true,
  "dynamicConfigs": [
    {
      "pinType": "Data",
      "direction": "Input",
      "nameTemplate": "Input {}",
      "dataType": "float64",
      "minCount": 2,
      "maxCount": 10,
      "canReorder": true
    }
  ]
}
```

### 2. 添加动态 Pin

```rust
#[tauri::command]
pub fn add_node_dynamic_pin(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
    pin_type: String,      // "data" 或 "exec"
    direction: String,     // "input" 或 "output"
) -> Result<serde_json::Value, String>
```

**返回示例**：
```json
{
  "pinId": "550e8400-e29b-41d4-a716-446655440000",
  "name": "Input 3",
  "type": "Data",
  "direction": "Input"
}
```

### 3. 移除动态 Pin

```rust
#[tauri::command]
pub fn remove_node_dynamic_pin(
    app: AppHandle,
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
    pin_id: String,
) -> Result<(), String>
```

## 前端 TypeScript 封装

### 1. 类型定义

```typescript
// types/dynamicPin.ts

export type PinType = 'data' | 'exec';
export type PinDirection = 'input' | 'output';

export interface DynamicPinConfig {
  pinType: PinType;
  direction: PinDirection;
  nameTemplate: string;
  dataType: string;
  minCount: number;
  maxCount: number | null;
  canReorder: boolean;
}

export interface NodeDynamicCapability {
  canAddPins: boolean;
  dynamicConfigs: DynamicPinConfig[];
}

export interface DynamicPinInfo {
  pinId: string;
  name: string;
  type: string;
  direction: string;
}
```

### 2. API 服务

```typescript
// services/dynamicPinService.ts

import { invoke } from '@tauri-apps/api/core';
import type { 
  NodeDynamicCapability, 
  DynamicPinInfo,
  PinType,
  PinDirection 
} from '../types/dynamicPin';

export class DynamicPinService {
  /**
   * 获取节点的动态能力
   */
  static async getNodeDynamicConstraints(
    subgraphId: string,
    nodeId: string
  ): Promise<NodeDynamicCapability> {
    return await invoke('get_node_dynamic_constraints', {
      subgraphId,
      nodeId,
    });
  }

  /**
   * 添加动态 Pin
   */
  static async addDynamicPin(
    subgraphId: string,
    nodeId: string,
    pinType: PinType,
    direction: PinDirection
  ): Promise<DynamicPinInfo> {
    return await invoke('add_node_dynamic_pin', {
      subgraphId,
      nodeId,
      pinType,
      direction,
    });
  }

  /**
   * 移除动态 Pin
   */
  static async removeDynamicPin(
    subgraphId: string,
    nodeId: string,
    pinId: string
  ): Promise<void> {
    return await invoke('remove_node_dynamic_pin', {
      subgraphId,
      nodeId,
      pinId,
    });
  }

  /**
   * 检查是否可以添加更多 Pin
   */
  static canAddMorePins(
    capability: NodeDynamicCapability,
    currentCount: number,
    pinType: PinType,
    direction: PinDirection
  ): boolean {
    const config = capability.dynamicConfigs.find(
      c => c.pinType === pinType && c.direction === direction
    );
    
    if (!config) return false;
    if (config.maxCount === null) return true;
    
    return currentCount < config.maxCount;
  }

  /**
   * 检查是否可以移除 Pin
   */
  static canRemovePin(
    capability: NodeDynamicCapability,
    currentCount: number,
    pinType: PinType,
    direction: PinDirection
  ): boolean {
    const config = capability.dynamicConfigs.find(
      c => c.pinType === pinType && c.direction === direction
    );
    
    if (!config) return false;
    
    return currentCount > config.minCount;
  }
}
```

### 3. React Hook

```typescript
// hooks/useDynamicPins.ts

import { useState, useEffect } from 'react';
import { DynamicPinService } from '../services/dynamicPinService';
import type { NodeDynamicCapability, PinType, PinDirection } from '../types/dynamicPin';

export function useDynamicPins(subgraphId: string, nodeId: string) {
  const [capability, setCapability] = useState<NodeDynamicCapability | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 加载动态能力
  useEffect(() => {
    loadCapability();
  }, [subgraphId, nodeId]);

  const loadCapability = async () => {
    try {
      setLoading(true);
      const cap = await DynamicPinService.getNodeDynamicConstraints(
        subgraphId,
        nodeId
      );
      setCapability(cap);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load capability');
      setCapability(null);
    } finally {
      setLoading(false);
    }
  };

  // 添加 Pin
  const addPin = async (pinType: PinType, direction: PinDirection) => {
    try {
      setLoading(true);
      const result = await DynamicPinService.addDynamicPin(
        subgraphId,
        nodeId,
        pinType,
        direction
      );
      setError(null);
      return result;
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to add pin';
      setError(message);
      throw new Error(message);
    } finally {
      setLoading(false);
    }
  };

  // 移除 Pin
  const removePin = async (pinId: string) => {
    try {
      setLoading(true);
      await DynamicPinService.removeDynamicPin(subgraphId, nodeId, pinId);
      setError(null);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to remove pin';
      setError(message);
      throw new Error(message);
    } finally {
      setLoading(false);
    }
  };

  // 检查是否可以添加
  const canAddPin = (
    currentCount: number,
    pinType: PinType,
    direction: PinDirection
  ): boolean => {
    if (!capability) return false;
    return DynamicPinService.canAddMorePins(
      capability,
      currentCount,
      pinType,
      direction
    );
  };

  // 检查是否可以移除
  const canRemovePin = (
    currentCount: number,
    pinType: PinType,
    direction: PinDirection
  ): boolean => {
    if (!capability) return false;
    return DynamicPinService.canRemovePin(
      capability,
      currentCount,
      pinType,
      direction
    );
  };

  return {
    capability,
    loading,
    error,
    addPin,
    removePin,
    canAddPin,
    canRemovePin,
    reload: loadCapability,
  };
}
```

### 4. React 组件示例

```typescript
// components/DynamicAddNode.tsx

import React from 'react';
import { useDynamicPins } from '../hooks/useDynamicPins';
import type { Node, Pin } from '../types/graph';

interface DynamicAddNodeProps {
  subgraphId: string;
  node: Node;
  onNodeUpdate: (node: Node) => void;
}

export function DynamicAddNode({ 
  subgraphId, 
  node, 
  onNodeUpdate 
}: DynamicAddNodeProps) {
  const {
    capability,
    loading,
    error,
    addPin,
    removePin,
    canAddPin,
    canRemovePin,
  } = useDynamicPins(subgraphId, node.id);

  const handleAddInput = async () => {
    try {
      const result = await addPin('data', 'input');
      console.log('Added pin:', result);
      
      // 更新节点（触发重新渲染）
      // 实际应用中，这应该通过状态管理或事件系统处理
      onNodeUpdate({ ...node });
    } catch (err) {
      console.error('Failed to add input:', err);
    }
  };

  const handleRemoveInput = async (pinId: string) => {
    try {
      await removePin(pinId);
      console.log('Removed pin:', pinId);
      
      // 更新节点
      onNodeUpdate({ ...node });
    } catch (err) {
      console.error('Failed to remove input:', err);
    }
  };

  if (!capability?.canAddPins) {
    // 不支持动态 Pin，渲染普通节点
    return <StandardNode node={node} />;
  }

  const inputCount = node.inputs.length;
  const canAdd = canAddPin(inputCount, 'data', 'input');
  const canRemove = canRemovePin(inputCount, 'data', 'input');

  return (
    <div className="dynamic-node">
      <div className="node-header">
        <h3>{node.title}</h3>
        {capability && (
          <span className="pin-count">
            {inputCount}/{capability.dynamicConfigs[0]?.maxCount || '∞'}
          </span>
        )}
      </div>

      <div className="node-body">
        {/* 输入 Pins */}
        <div className="node-inputs">
          {node.inputs.map((input, index) => (
            <div key={input.id} className="pin-row">
              <Pin pin={input} />
              
              {/* 只有超过最小数量的 Pin 才能移除 */}
              {canRemove && index >= 2 && (
                <button
                  className="remove-pin-btn"
                  onClick={() => handleRemoveInput(input.id)}
                  disabled={loading}
                  title="Remove input"
                >
                  ✕
                </button>
              )}
            </div>
          ))}

          {/* 添加按钮 */}
          {canAdd && (
            <button
              className="add-pin-btn"
              onClick={handleAddInput}
              disabled={loading}
            >
              + Add Input
            </button>
          )}
        </div>

        {/* 输出 Pins */}
        <div className="node-outputs">
          {node.outputs.map(output => (
            <Pin key={output.id} pin={output} />
          ))}
        </div>
      </div>

      {/* 错误提示 */}
      {error && (
        <div className="node-error">
          {error}
        </div>
      )}

      {/* 加载状态 */}
      {loading && (
        <div className="node-loading">
          Processing...
        </div>
      )}
    </div>
  );
}

// 标准节点组件（不支持动态 Pin）
function StandardNode({ node }: { node: Node }) {
  return (
    <div className="standard-node">
      <div className="node-header">
        <h3>{node.title}</h3>
      </div>
      <div className="node-body">
        <div className="node-inputs">
          {node.inputs.map(input => (
            <Pin key={input.id} pin={input} />
          ))}
        </div>
        <div className="node-outputs">
          {node.outputs.map(output => (
            <Pin key={output.id} pin={output} />
          ))}
        </div>
      </div>
    </div>
  );
}
```

### 5. 样式示例

```css
/* styles/dynamicNode.css */

.dynamic-node {
  position: relative;
  background: #2d2d2d;
  border: 2px solid #444;
  border-radius: 8px;
  min-width: 200px;
}

.node-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: #1a1a1a;
  border-bottom: 1px solid #444;
}

.node-header h3 {
  margin: 0;
  font-size: 14px;
  color: #fff;
}

.pin-count {
  font-size: 12px;
  color: #888;
}

.node-body {
  padding: 12px;
}

.pin-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.remove-pin-btn {
  width: 20px;
  height: 20px;
  padding: 0;
  background: #ff4444;
  border: none;
  border-radius: 4px;
  color: white;
  cursor: pointer;
  font-size: 12px;
  opacity: 0.7;
  transition: opacity 0.2s;
}

.remove-pin-btn:hover {
  opacity: 1;
}

.remove-pin-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.add-pin-btn {
  width: 100%;
  padding: 6px;
  background: #0078d4;
  border: 1px dashed #0078d4;
  border-radius: 4px;
  color: white;
  cursor: pointer;
  font-size: 12px;
  transition: background 0.2s;
}

.add-pin-btn:hover {
  background: #106ebe;
}

.add-pin-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.node-error {
  padding: 8px;
  background: #ff444420;
  border-top: 1px solid #ff4444;
  color: #ff6666;
  font-size: 12px;
}

.node-loading {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.7);
  color: white;
  font-size: 12px;
  border-radius: 8px;
}
```

## 完整使用流程

### 1. 用户创建节点

```typescript
// 创建 dynamic_add 节点
const node = await createNode(subgraphId, {
  type: 'dynamic_add',
  title: 'Add (Dynamic)',
  position: { x: 100, y: 100 }
});

// 节点初始有 2 个输入：Input 1, Input 2
console.log(node.inputs.length); // 2
```

### 2. 检查动态能力

```typescript
const capability = await DynamicPinService.getNodeDynamicConstraints(
  subgraphId,
  node.id
);

console.log(capability);
// {
//   canAddPins: true,
//   dynamicConfigs: [{
//     pinType: "Data",
//     direction: "Input",
//     minCount: 2,
//     maxCount: 10,
//     ...
//   }]
// }
```

### 3. 添加输入

```typescript
// 添加第 3 个输入
const newPin = await DynamicPinService.addDynamicPin(
  subgraphId,
  node.id,
  'data',
  'input'
);

console.log(newPin);
// {
//   pinId: "...",
//   name: "Input 3",
//   type: "Data",
//   direction: "Input"
// }

// 重新获取节点以查看更新
const updatedNode = await getNode(subgraphId, node.id);
console.log(updatedNode.inputs.length); // 3
```

### 4. 连接和执行

```typescript
// 连接常量到新的输入
await connectPins(
  subgraphId,
  constantNode.outputs[0].id,
  newPin.pinId
);

// 执行图
const result = await executeGraph(subgraphId);
console.log(result); // Sum = const1 + const2 + const3
```

### 5. 移除输入

```typescript
// 移除第 3 个输入
await DynamicPinService.removeDynamicPin(
  subgraphId,
  node.id,
  newPin.pinId
);

// 节点恢复到 2 个输入
const updatedNode = await getNode(subgraphId, node.id);
console.log(updatedNode.inputs.length); // 2
```

## 事件监听

```typescript
// 监听节点更新事件
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen('project-event', (event) => {
  const { eventType, data } = event.payload;
  
  if (eventType === 'NodesUpdated') {
    console.log('Nodes updated:', data.nodes);
    // 更新 UI
    updateNodesInCanvas(data.nodes);
  }
});

// 清理
unlisten();
```

## 错误处理

```typescript
try {
  await DynamicPinService.addDynamicPin(
    subgraphId,
    nodeId,
    'data',
    'input'
  );
} catch (error) {
  if (error.message.includes('Cannot add more pins')) {
    showNotification('Maximum number of inputs reached', 'warning');
  } else if (error.message.includes('Node not found')) {
    showNotification('Node not found', 'error');
  } else {
    showNotification('Failed to add input', 'error');
  }
}
```

## 最佳实践

1. **缓存动态能力**：避免频繁调用 `get_node_dynamic_constraints`
2. **乐观更新**：先更新 UI，失败时回滚
3. **防抖处理**：避免用户快速点击导致多次调用
4. **错误提示**：清晰地告知用户操作失败的原因
5. **加载状态**：显示加载指示器，提升用户体验

## 相关文档

- [动态 Pin 总结](./DYNAMIC_PIN_SUMMARY.md)
- [动态 Pin Add 示例](./DYNAMIC_PIN_ADD_EXAMPLE.md)
