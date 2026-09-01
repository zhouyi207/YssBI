/**
 * 通用类型定义
 * 用于替代 any 的语义化类型
 */

/** Pin 的默认值/用户值（来自后端或用户输入，类型不定） */
export type PinValue = unknown;

/** 通用 JSON 可序列化值 */
export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };
