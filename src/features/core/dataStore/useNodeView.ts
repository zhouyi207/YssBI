/**
 * useNodeView - 单节点订阅 Hook
 *
 * 为画布上的「每个节点」提供一个**仅订阅自身切片**的视图：
 *   - `nodes[nodeId]`            位置 / 标题 / 类型 / 参数
 *   - `nodePins[nodeId]` + 各 `pins[pid]`        输入/输出 Pin
 *   - 各 `pinConnections[pid]`   连接状态（派生 links）
 *
 * 这样一次图变更只会让「受影响的节点」重新渲染，而不再触发整图
 * `deserializeGraph` + 全量节点重渲染。返回的 `UINode` 对象在依赖未变时保持
 * 引用稳定，从而让 `Node` 的 `React.memo`（`prev.node === next.node`）真正生效。
 */
import { useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import type { Pin } from '@/shared/types/domain';
import type { UINode } from '@/shared/types/ui';
import { useGraphDataStore } from './graphDataStore';
import { resolveNodeViewMeta } from './serialization';

const EMPTY_IDS: string[] = [];

/**
 * 连接 id 形如 `fromPinId->toPinId`，据此解析出「另一端」的 pin id，
 * 无需订阅整张 connections 表。
 */
function otherEndpoint(connId: string, pinId: string): string {
  const sep = connId.indexOf('->');
  if (sep < 0) return connId;
  const from = connId.slice(0, sep);
  const to = connId.slice(sep + 2);
  return from === pinId ? to : from;
}

export function useNodeView(nodeId: string): UINode | null {
  const nodeData = useGraphDataStore((s) => s.nodes[nodeId]);

  // 订阅该节点的 pin 对象数组：useShallow 逐元素比较引用，
  // 仅当某个 pin 对象或 pin 集合变化时才更新。
  const pinObjs = useGraphDataStore(
    useShallow((s) => (s.nodePins[nodeId] ?? EMPTY_IDS).map((pid) => s.pins[pid])),
  );

  // 订阅每个 pin 的连接 id 数组（store 在连接变化时创建新数组）。
  const pinConns = useGraphDataStore(
    useShallow((s) => (s.nodePins[nodeId] ?? EMPTY_IDS).map((pid) => s.pinConnections[pid])),
  );

  return useMemo(() => {
    if (!nodeData) return null;

    const meta = resolveNodeViewMeta(nodeData);
    const inputs: Pin[] = [];
    const outputs: Pin[] = [];

    for (let i = 0; i < pinObjs.length; i++) {
      const p = pinObjs[i];
      if (!p) continue;
      const conns = pinConns[i] ?? EMPTY_IDS;
      const links = conns.map((cid) => otherEndpoint(cid, p.id));
      const pin: Pin = { ...p, links };
      if (p.direction === 'output') outputs.push(pin);
      else inputs.push(pin);
    }

    const view: UINode = {
      id: nodeData.id,
      nodeType: meta.nodeType,
      category: meta.category,
      title: meta.title,
      uiStyle: meta.uiStyle,
      description: meta.description,
      position: nodeData.position ?? { x: 0, y: 0 },
      isInternal: nodeData.isInternal,
      paramsKind: nodeData.paramsKind,
      variableId: nodeData.variableId,
      variableName: nodeData.variableName,
      variableType: nodeData.variableType,
      subGraphId: nodeData.subGraphId,
      dataframeId: nodeData.dataframeId,
      dataframeName: nodeData.dataframeName,
      inputs,
      outputs,
    };
    return view;
  }, [nodeData, pinObjs, pinConns]);
}
