/**
 * dataStore 项目相关辅助函数
 * 替代原 @/features/core/project 中的 helpers
 */

import { useMemo } from 'react';
import { LoadStatus } from '@/shared/types/ui/common';
import { ProjectData } from '@/shared/types';
import { useProjectIOStore } from './projectIOStore';
import { useGraphMetaStore } from './graphMetaStore';
import { useGraphDataStore } from './graphDataStore';

const EMPTY_CANVAS = { x: 0, y: 0, scale: 1 };

/**
 * 从 GraphMetaStore + GraphDataStore 获取指定 graph 的完整数据（用于 openGraph 等）
 */
export function getGraphById(graphId: string): any | null {
  const meta = useGraphMetaStore.getState().graphs[graphId];
  if (!meta) return null;

  const dataState = useGraphDataStore.getState();
  const nodeIds = dataState.graphNodes[graphId] ?? [];
  const nodes = nodeIds.map((nid) => dataState.nodes[nid]).filter(Boolean);
  const pins = nodeIds.flatMap((nid) => (dataState.nodePins[nid] ?? []).map((pid) => dataState.pins[pid]).filter(Boolean));
  const connIds = new Set<string>();
  pins.forEach((p) => {
    (dataState.pinConnections[p?.id] ?? []).forEach((cid) => connIds.add(cid));
  });
  const connections = Array.from(connIds).map((cid) => dataState.connections[cid]).filter(Boolean);

  return { ...meta, nodes, pins, connections, canvas: { x: 0, y: 0, scale: 1 } };
}

/**
 * 获取所有 graphs（按 graphOrder 顺序）
 */
export function getGraphs(): Record<string, any> {
  const metaStore = useGraphMetaStore.getState();
  const result: Record<string, any> = {};
  for (const gid of metaStore.graphOrder) {
    const g = getGraphById(gid);
    if (g) result[gid] = g;
  }
  return result;
}

/**
 * React Hook: 订阅指定 graph 的数据（用于需要响应式更新的组件）
 * 注意：selector 必须返回稳定引用，避免 "getSnapshot should be cached" 无限循环
 */
export function useGraphData(activeTabId: string | null) {
  const meta = useGraphMetaStore((s) => (activeTabId ? s.graphs[activeTabId] : null));
  const graphNodeIds = useGraphDataStore((s) =>
    activeTabId && s.graphNodes[activeTabId] ? s.graphNodes[activeTabId] : null
  );
  const nodesRecord = useGraphDataStore((s) => s.nodes);
  const nodePinsRecord = useGraphDataStore((s) => s.nodePins);
  const pinsRecord = useGraphDataStore((s) => s.pins);
  const pinConnectionsRecord = useGraphDataStore((s) => s.pinConnections);
  const connectionsRecord = useGraphDataStore((s) => s.connections);

  return useMemo(() => {
    if (!activeTabId || !meta || !graphNodeIds) return null;

    const nodeIds = graphNodeIds;
    const nodes = nodeIds.map((nid) => nodesRecord[nid]).filter(Boolean);
    const pins = nodeIds.flatMap((nid) =>
      (nodePinsRecord[nid] ?? []).map((pid) => pinsRecord[pid]).filter(Boolean)
    );
    const connIds = new Set<string>();
    pins.forEach((p) =>
      (pinConnectionsRecord[p?.id] ?? []).forEach((cid) => connIds.add(cid))
    );
    const connections = Array.from(connIds).map((cid) => connectionsRecord[cid]).filter(Boolean);

    return { ...meta, nodes, pins, connections, canvas: EMPTY_CANVAS };
  }, [
    activeTabId,
    meta,
    graphNodeIds,
    nodesRecord,
    nodePinsRecord,
    pinsRecord,
    pinConnectionsRecord,
    connectionsRecord,
  ]);
}

/**
 * 初始化时从后端同步项目状态
 * 应该在应用启动时调用一次
 *
 * - 如果 Project 已 Ready，会触发同步
 * - 如果已经 Ready，直接返回当前数据
 */
export async function initProjectSync(): Promise<ProjectData | null> {
  const { status, syncFromBackend, exportSnapshot } = useProjectIOStore.getState();

  if (status === LoadStatus.Ready) {
    return exportSnapshot();
  }

  return await syncFromBackend();
}

/**
 * 获取当前项目路径
 */
export function getCurrentProjectPath(): string | null {
  const { status, currentPath } = useProjectIOStore.getState();

  if (status !== LoadStatus.Ready) {
    return null;
  }

  return currentPath;
}

/**
 * 检查项目是否已加载
 */
export function isProjectLoaded(): boolean {
  const { status } = useProjectIOStore.getState();
  return status === LoadStatus.Ready;
}

/**
 * 获取项目数据（只读）
 */
export function getProjectData(): ProjectData {
  const { status, exportSnapshot } = useProjectIOStore.getState();

  if (status !== LoadStatus.Ready) {
    return {
      variables: {},
      graphs: {},
      databases: {},
      metadata: {
        exportTime: '',
        appVersion: '',
      },
    };
  }

  return exportSnapshot();
}
