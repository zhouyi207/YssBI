import type { LoadStatus } from "../ui";
import type { ProjectData } from "../domain/project";
import type { Graph } from "../domain/graph";

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
  payload: unknown;
}

/**
 * useProjectSync 配置
 */
export interface UseProjectSyncOptions {
  enabled?: boolean;
  onProjectLoaded?: (data: ProjectData, path: string | null) => void;
  onProjectCleared?: () => void;
  onProjectSaved?: (path: string) => void;
  onEventCreated?: (id: string, data: Graph) => void;
  onFunctionCreated?: (id: string, data: Graph) => void;
}
