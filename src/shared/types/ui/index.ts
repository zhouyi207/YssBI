/**
 * UI Types
 * 
 * UI 状态类型定义
 * 这些类型只在前端使用，不会发送到后端
 * 
 * 用途：
 * - UI 组件状态
 * - 用户交互状态
 * - 视图层数据
 * 
 * 特点：
 * - 前端专用
 * - 包含 UI 特定的属性
 * - 不需要序列化到后端
 */

export * from './common';
export * from './editor';
export * from '../layout/layout';
export * from './execution';
