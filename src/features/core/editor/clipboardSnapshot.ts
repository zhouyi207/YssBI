import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import type { ClipboardEntry, ClipboardPinEntry, ClipboardSnapshot } from './stores/useClipboardStore';

export function buildClipboardSnapshot(nodeIds: string[]): ClipboardSnapshot | null {
  if (nodeIds.length === 0) return null;

  const dataStore = useGraphDataStore.getState();
  const allSelectedPinIds = new Set<string>();
  const entries: ClipboardEntry[] = [];

  for (const nodeId of nodeIds) {
    const node = dataStore.nodes[nodeId];
    if (!node || node.isInternal) continue;

    const pinIds = dataStore.nodePins[nodeId] ?? [];
    const pins: ClipboardPinEntry[] = [];

    for (const pinId of pinIds) {
      const pin = dataStore.pins[pinId];
      if (!pin) continue;
      allSelectedPinIds.add(pinId);
      pins.push({
        pinId: pin.id,
        name: pin.name,
        direction: pin.direction as 'input' | 'output',
        userValue: pin.userValue,
      });
    }

    const params: ClipboardEntry['params'] = {};
    if (node.variableId) params.variableId = node.variableId;
    if (node.variableName) params.variableName = node.variableName;
    if (node.variableType) params.variableType = node.variableType;
    if (node.subGraphId) params.subGraphId = node.subGraphId;
    if (node.dataframeId) params.dataframeId = node.dataframeId;

    entries.push({
      nodeType: node.nodeType,
      position: { x: node.position.x, y: node.position.y },
      params: Object.keys(params).length > 0 ? params : undefined,
      pins,
    });
  }

  if (entries.length === 0) return null;

  const internalConnections: ClipboardSnapshot['internalConnections'] = [];
  const seenConnIds = new Set<string>();

  for (const pinId of allSelectedPinIds) {
    const connIds = dataStore.pinConnections[pinId] ?? [];
    for (const connId of connIds) {
      if (seenConnIds.has(connId)) continue;
      seenConnIds.add(connId);
      const conn = dataStore.connections[connId];
      if (!conn) continue;
      if (allSelectedPinIds.has(conn.from) && allSelectedPinIds.has(conn.to)) {
        internalConnections.push({ fromPin: conn.from, toPin: conn.to });
      }
    }
  }

  return { entries, internalConnections };
}
