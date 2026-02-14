/**
 * 前后端 DTO 转换器统一导出
 * 
 * 这个文件提供了所有前后端数据转换的辅助函数
 */

export { NodeConverter } from './node';
export { PinConverter } from './pin';
export { ConnectionConverter } from './connection';
export { GraphConverter } from './graph';
export { VariableConverter } from './variable';
export { ProjectDataConverter } from './project';

// 数据库转换器
import type {
  CsvSource,
  JsonSource,
  ExcelSource,
  SqlSource,
  ApiSource,
  TransformSource,
  InlineSource,
  DataSourceConfig,
} from './database';

export const DatabaseConverter = {
  fromDTO(dto: DataSourceConfig): DataSourceConfig {
    return dto;
  },

  toDTO(config: DataSourceConfig): DataSourceConfig {
    return config;
  },
};

// 导出所有转换器的类型
export type Converter<T, D = T> = {
  fromDTO(dto: D): T;
  toDTO(data: T): D;
};
