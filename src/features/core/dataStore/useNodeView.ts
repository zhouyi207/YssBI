/**
 * useNodeView - 单节点订阅 Hook
 *
 * 为画布上的「每个节点」提供一个**仅订阅自身切片**的视图：
 *   - `nodes[nodeId]`            位置 / 标题 / 类型 / 参数
 *   - `nodePins[nodeId]` + 各 `pins[pid]`        输入/输出 Pin
 *   - 各 `pinConnections[pid]`   连接状态（派生 connected / connectionIds）
 */
import { useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import type { PinView } from '@/shared/types/store/graph';
import type { UINode } from '@/shared/types/ui';
import { useGraphDataStore } from './graphDataStore';
import { derivePinConnectionView } from './pinLinks';
import { resolveNodeViewMeta } from './serialization';

const EMPTY_IDS: string[] = [];

export function useNodeView(nodeId: string): UINode | null {
  const nodeData = useGraphDataStore((s) => s.nodes[nodeId]);

  const pinObjs = useGraphDataStore(
    useShallow((s) => (s.nodePins[nodeId] ?? EMPTY_IDS).map((pid) => s.pins[pid])),
  );

  const pinConns = useGraphDataStore(
    useShallow((s) => (s.nodePins[nodeId] ?? EMPTY_IDS).map((pid) => s.pinConnections[pid])),
  );

  return useMemo(() => {
    if (!nodeData) return null;

    const meta = resolveNodeViewMeta(nodeData);
    const inputs: PinView[] = [];
    const outputs: PinView[] = [];

    for (let i = 0; i < pinObjs.length; i++) {
      const p = pinObjs[i];
      if (!p) continue;
      const connectionView = derivePinConnectionView(pinConns[i]);
      const pin: PinView = { ...p, ...connectionView };
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
      inputs,
      outputs,
    };
    return view;
  }, [nodeData, pinObjs, pinConns]);
}
