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
  payload: unknown;
}
