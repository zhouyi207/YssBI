/**
 * 通用类型定义
 * 用于替代 any 的语义化类型
 */

/** 通用 JSON 可序列化值 */
export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };
