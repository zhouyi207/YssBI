/**
 * dataStore 项目相关辅助函数
 * 替代原 @/features/core/project 中的 helpers
 */

import { useMemo, useRef } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { LoadStatus } from '@/shared/types/ui/common';
import { ProjectData, GraphData } from '@/shared/types';
import { useProjectIOStore } from './projectIOStore';
import { useGraphDataStore } from './graphDataStore';
import { useGraphMetaStore } from './graphMetaStore';
import { resourceKey, useResourceStore } from '@/features/core/resource';
import { getViewport } from '@/features/core/viewport';

function isPresent<T>(value: T | null | undefined): value is T {
  return value != null;
}

function getGraphMetaFromResourceStore(graphPath: string): { path: string; name: string; type: 'event' | 'function' } | null {
  const resources = useResourceStore.getState().resources;
  const eventMeta = resources[resourceKey({ id: graphPath, kind: 'event' })];
  if (eventMeta?.exists) {
    return { path: graphPath, name: eventMeta.name, type: 'event' };
  }
  const functionMeta = resources[resourceKey({ id: graphPath, kind: 'function' })];
  if (functionMeta?.exists) {
    return { path: graphPath, name: functionMeta.name, type: 'function' };
  }
  return null;
}

/**
 * 从 ResourceStore + GraphDataStore 获取指定 graph 的完整数据（用于 openGraph 等）
 */
export function getGraphByPath(graphPath: string): GraphData | null {
  const meta = getGraphMetaFromResourceStore(graphPath);
  if (!meta) return null;

  const dataState = useGraphDataStore.getState();
  const graphMeta = useGraphMetaStore.getState().graphs[graphPath];
  const nodeIds = dataState.getGraphNodeIds(graphPath);
  const nodes = nodeIds.map((nid) => dataState.getGraphNode(graphPath, nid)).filter(isPresent);
  const pins = nodeIds.flatMap((nid) =>
    dataState.getGraphNodePins(graphPath, nid).map((pid) => dataState.getGraphPin(graphPath, pid)).filter(isPresent),
  );
  const connIds = new Set<string>();
  pins.forEach((p) => {
    (p ? dataState.getGraphPinConnections(graphPath, p.id) : []).forEach((cid) => connIds.add(cid));
  });
  const connections = Array.from(connIds)
    .map((cid) => dataState.getGraphConnection(graphPath, cid))
    .filter(isPresent);

  return {
    ...meta,
    functionInputs: graphMeta?.functionInputs ?? [],
    functionOutputs: graphMeta?.functionOutputs ?? [],
    nodes,
    pins,
    connections,
    canvas: getViewport(graphPath),
  };
}

/**
 * 获取所有 graphs（按 ResourceStore graphOrder 顺序）
 */
export function getGraphs(): Record<string, GraphData> {
  const graphOrder = useResourceStore.getState().graphOrder;
  const result: Record<string, GraphData> = {};
  for (const gid of graphOrder) {
    const g = getGraphByPath(gid);
    if (g) result[gid] = g;
  }
  return result;
}

/**
 * React Hook: 订阅指定 graph 的数据（用于需要响应式更新的组件）
 * 注意：selector 必须返回稳定引用，避免 "getSnapshot should be cached" 无限循环
 */
export function useGraphData(activeTabId: string | null) {
  const meta = useResourceStore((s) => {
    if (!activeTabId) return null;
    const eventMeta = s.resources[resourceKey({ id: activeTabId, kind: 'event' })];
    if (eventMeta?.exists) {
      return { path: activeTabId, name: eventMeta.name, type: 'event' as const };
    }
    const functionMeta = s.resources[resourceKey({ id: activeTabId, kind: 'function' })];
    if (functionMeta?.exists) {
      return { path: activeTabId, name: functionMeta.name, type: 'function' as const };
    }
    return null;
  });

  // 只提取当前图的 node 数组，useShallow 对比每个 node 引用，
  // 其他图变化时 node 引用不变 → 不触发 re-render
  const graphNodes = useGraphDataStore(
    useShallow((s) => {
      if (!activeTabId || !s.hasGraph(activeTabId)) return null;
      const ids = s.getGraphNodeIds(activeTabId);
      return ids.map((nid) => s.getGraphNode(activeTabId, nid)).filter(isPresent);
    })
  );

  const graphPins = useGraphDataStore(
    useShallow((s) => {
      if (!activeTabId || !s.hasGraph(activeTabId)) return null;
      const nodeIds = s.getGraphNodeIds(activeTabId);
      return nodeIds.flatMap((nid) =>
        s.getGraphNodePins(activeTabId, nid).map((pid) => s.getGraphPin(activeTabId, pid)).filter(isPresent)
      );
    })
  );

  const graphConnections = useGraphDataStore(
    useShallow((s) => {
      if (!activeTabId || !s.hasGraph(activeTabId)) return null;
      const nodeIds = s.getGraphNodeIds(activeTabId);
      const connIds = new Set<string>();
      for (const nid of nodeIds) {
        for (const pid of s.getGraphNodePins(activeTabId, nid)) {
          for (const cid of s.getGraphPinConnections(activeTabId, pid)) {
            connIds.add(cid);
          }
        }
      }
      return Array.from(connIds)
        .map((cid) => s.getGraphConnection(activeTabId, cid))
        .filter(isPresent);
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
      connections: graphConnections ?? [],
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
