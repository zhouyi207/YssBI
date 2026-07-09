/**
 * Domain Types
 * 
 * 领域模型类型定义
 * 这些类型代表核心业务领域，与后端数据结构一致
 * 
 * 用途：
 * - 后端 API 响应
 * - 数据持久化
 * - 业务逻辑处理
 * 
 * 特点：
 * - 与后端保持一致
 * - 不包含 UI 特定的属性
 * - 可以直接序列化/反序列化
 */

export * from './ids';
export * from './node';
export * from './pin';
export * from './pinSemantics';
export * from './pinVisual';
export * from './connection';
export * from './graph';
export * from './graphResourcePath';
export * from './dataType';
export * from './typeSystem';
export * from './dataValue';
export * from './variable';
export * from './project';
export * from './database';
export * from './dataframe';
export * from './schema';
export * from './nodeRegister';
export * from './worksheet';
