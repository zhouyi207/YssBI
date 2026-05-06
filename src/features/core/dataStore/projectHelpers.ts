/**
 * dataStore 项目相关辅助函数
 * 替代原 @/features/core/project 中的 helpers
 */

import { useMemo, useRef } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { LoadStatus } from '@/shared/types/ui/common';
import { ProjectData, GraphData } from '@/shared/types';
import { useProjectIOStore } from './projectIOStore';
import { useGraphMetaStore } from './graphMetaStore';
import { useGraphDataStore } from './graphDataStore';
import { getViewport } from '@/features/core/viewport';

/**
 * 从 GraphMetaStore + GraphDataStore 获取指定 graph 的完整数据（用于 openGraph 等）
 */
export function getGraphById(graphId: string): GraphData | null {
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
  const connections = Array.from(connIds)
    .map((cid) => dataState.connections[cid])
    .filter(Boolean)
    .map((conn) => ({ fromPin: conn.from, toPin: conn.to }));

  return { ...meta, nodes, pins, connections: { connections }, canvas: getViewport(graphId) };
}

/**
 * 获取所有 graphs（按 graphOrder 顺序）
 */
export function getGraphs(): Record<string, GraphData> {
  const metaStore = useGraphMetaStore.getState();
  const result: Record<string, GraphData> = {};
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

  // 只提取当前图的 node 数组，useShallow 对比每个 node 引用，
  // 其他图变化时 node 引用不变 → 不触发 re-render
  const graphNodes = useGraphDataStore(
    useShallow((s) => {
      if (!activeTabId) return null;
      const ids = s.graphNodes[activeTabId];
      if (!ids) return null;
      return ids.map((nid) => s.nodes[nid]).filter(Boolean);
    })
  );

  const graphPins = useGraphDataStore(
    useShallow((s) => {
      if (!activeTabId) return null;
      const nodeIds = s.graphNodes[activeTabId];
      if (!nodeIds) return null;
      return nodeIds.flatMap((nid) =>
        (s.nodePins[nid] ?? []).map((pid) => s.pins[pid]).filter(Boolean)
      );
    })
  );

  const graphConnections = useGraphDataStore(
    useShallow((s) => {
      if (!activeTabId) return null;
      const nodeIds = s.graphNodes[activeTabId];
      if (!nodeIds) return null;
      const connIds = new Set<string>();
      for (const nid of nodeIds) {
        for (const pid of s.nodePins[nid] ?? []) {
          for (const cid of s.pinConnections[pid] ?? []) {
            connIds.add(cid);
          }
        }
      }
      return Array.from(connIds).map((cid) => s.connections[cid]).filter(Boolean);
    })
  );

  // 用 ref 缓存上一次结果，只在内容真正变化时返回新引用
  const prevRef = useRef<GraphData | null>(null);

  return useMemo(() => {
    if (!activeTabId || !meta || !graphNodes) {
      prevRef.current = null;
      return null;
    }

    const result: GraphData = {
      ...meta,
      nodes: graphNodes,
      pins: graphPins ?? [],
      connections: {
        connections: (graphConnections ?? []).map((conn) => ({ fromPin: conn.from, toPin: conn.to })),
      },
      canvas: getViewport(activeTabId),
    };
    prevRef.current = result;
    return result;
  }, [activeTabId, meta, graphNodes, graphPins, graphConnections]);
}

/**
 * 初始化时从后端同步项目状态
 * 应该在应用启动时调用一次
 *
 * - 如果 Project 已 Ready，会触发同步
 * - 如果已经 Ready，直接返回当前数据
 */
export async function initProjectSync(): Promise<ProjectData | null> {
  const { status, loadProject, exportSnapshot } = useProjectIOStore.getState();

  if (status === LoadStatus.Ready) {
    return exportSnapshot();
  }

  return await loadProject();
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
