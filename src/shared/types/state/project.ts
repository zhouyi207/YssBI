import type { LoadStatus } from "../ui";

/**
 * Project 初始化状态
 */
export interface ProjectState {
  status: LoadStatus;
  error: string | null;
}

/**
 * 项目事件类型（与后端 ProjectEvent 对应）
 */
export interface ProjectEventPayload {
  type: string;
  payload: any;
}

/**
 * useProjectSync 配置
 */
export interface UseProjectSyncOptions {
  enabled?: boolean;
  onProjectLoaded?: (data: any, path: string | null) => void;
  onProjectCleared?: () => void;
  onProjectSaved?: (path: string) => void;
  onEventCreated?: (id: string, data: any) => void;
  onFunctionCreated?: (id: string, data: any) => void;
  onMacroCreated?: (id: string, data: any) => void;
}
