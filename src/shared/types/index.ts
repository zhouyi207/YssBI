/**
 * Shared Types
 *
 * 类型系统组织结构：
 *
 * 1. domain/   - 领域模型（与后端一致）
 * 2. dto/      - IPC 数据传输对象
 * 3. ui/       - 跨模块通用 UI 契约
 * 4. settings/ - 跨模块设置契约
 *
 * 使用指南：
 *
 * - 后端 API 调用：使用 domain 类型
 * - 前后端传输：使用 dto wire 类型
 * - UI 组件状态：使用 ui 类型
 * - 应用设置：使用 settings 类型
 */

// ==================== Common Types ====================
export type { PinValue, JsonValue } from "./common";
export * from "./bayes";

// ==================== Domain Types ====================
// 领域模型 - 与后端数据结构一致
export * from "./domain";

// ==================== Settings ====================
// 设置相关类型
export * from "./settings";
