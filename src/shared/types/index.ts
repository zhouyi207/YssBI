/**
 * Shared Types
 *
 * 类型系统组织结构：
 *
 * 1. domain/   - 领域模型（与后端一致）
 * 2. dto/      - 数据传输对象和转换器
 * 3. ui/       - UI 相关类型（前端专用）
 * 4. state/    - 前端状态类型（Store/Hook）
 * 5. settings/ - 设置相关类型
 *
 * 使用指南：
 *
 * - 后端 API 调用：使用 domain 类型
 * - 前后端转换：使用 dto 转换器
 * - UI 组件状态：使用 ui 类型
 * - 应用设置：使用 settings 类型
 */

// ==================== Common Types ====================
export type { PinValue, JsonValue } from "./common";
export * from "./bayes";

// ==================== Domain Types ====================
// 领域模型 - 与后端数据结构一致
export * from "./domain";

// ==================== UI Types ====================
// UI 状态类型 - 前端专用
export type { LoadStatus, ExecutionStatus } from "./ui";
export type { Position, Size, Rect } from "./ui/common";

// ==================== Store Types ====================
// NodeData, PinData, ConnectionData, GraphData（NodeId 等 ID 类型从 domain 导出）
export * from "./store";

// ==================== State Types ====================
// 前端状态类型 - Store/Hook 专用
export * from "./state";

// ==================== Settings ====================
// 设置相关类型
export * from "./settings";
