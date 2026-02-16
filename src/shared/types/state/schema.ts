import type { LoadStatus } from "../ui";

/**
 * Schema 初始化状态
 */
export interface SchemaState {
  status: LoadStatus;
  error: string | null;
}
