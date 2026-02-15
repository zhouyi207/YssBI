/// hooks —— 生命周期 + 组合逻辑（重点）

import { useProjectStore } from './project.store';
import { ProjectState } from '@/shared/types';



/**
 * Project 初始化 Hook
 *
 * 语义：
 * - 首次使用时自动触发初始化
 * - 返回标准的 ProjectState（status + error）
 * - 不自动同步，需要手动调用 syncFromBackend
 */
export function useProject(): ProjectState {
  const status = useProjectStore((s) => s.status);
  const error = useProjectStore((s) => s.error);

  return {
    status,
    error,
  };
}

