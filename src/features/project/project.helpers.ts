/// helpers —— 非 React 的纯函数

import { useProjectStore } from './project.store';
import { LoadStatus } from '@/shared/types/loadStatus';
import { ProjectData } from '@/shared/types/editor';

/**
 * 初始化时从后端同步项目状态
 * 应该在应用启动时调用一次
 *
 * - 如果 Project 未 Ready，会触发同步
 * - 如果已经 Ready，直接返回当前数据
 */
export async function initProjectSync(): Promise<ProjectData | null> {
  const { status, syncFromBackend } = useProjectStore.getState();

  // 如果已经加载，直接返回当前数据
  if (status === LoadStatus.Ready) {
    const { events, functions, macros, globalVariables, dataframes } = useProjectStore.getState();
    return {
      events,
      functions,
      macros,
      globalVariables,
      dataframes,
      metadata: {
        exportTime: new Date().toISOString(),
        appVersion: "0.1.0"
      }
    };
  }

  // 否则触发同步
  return await syncFromBackend();
}

/**
 * 获取当前项目路径
 *
 * - Project 未 Ready 时返回 null
 */
export function getCurrentProjectPath(): string | null {
  const { status, currentPath } = useProjectStore.getState();

  if (status !== LoadStatus.Ready) {
    return null;
  }

  return currentPath;
}

/**
 * 检查项目是否已加载
 */
export function isProjectLoaded(): boolean {
  const { status } = useProjectStore.getState();
  return status === LoadStatus.Ready;
}

/**
 * 获取项目数据（只读）
 *
 * - Project 未 Ready 时返回空对象
 */
export function getProjectData(): ProjectData {
  const { status, events, functions, macros, globalVariables, dataframes } = useProjectStore.getState();

  if (status !== LoadStatus.Ready) {
    return {
      events: {},
      functions: {},
      macros: {},
      globalVariables: {},
      dataframes: {},
      metadata: {
        exportTime: '',
        appVersion: ''
      }
    };
  }

  return {
    events,
    functions,
    macros,
    globalVariables,
    dataframes,
    metadata: {
      exportTime: new Date().toISOString(),
      appVersion: "0.1.0"
    }
  };
}
