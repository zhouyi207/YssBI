/**
 * 变量定义类型
 *
 * 定义项目中变量的完整结构，包括元数据、值配置和作用域。
 * 与后端 Rust 类型保持同步。
 */

// ==================== 变量数据类型 ====================

/** 变量数据类型 */
export type VariableDataType =
  | "int"
  | "float"
  | "bool"
  | "string"
  | "object"
  | "array"
  | "dataframe"
  | "any";

// ==================== 变量作用域 ====================

/** 全局作用域 */
export interface GlobalScope {
  type: "global";
}

/** 函数作用域 */
export interface FunctionScope {
  type: "function";
  function_id: string;
}

/** 宏作用域 */
export interface MacroScope {
  type: "macro";
  macro_id: string;
}

/** 变量作用域 */
export type VariableScope = GlobalScope | FunctionScope | MacroScope;

// ==================== 数据来源配置 ====================

/** CSV 数据源 */
export interface CsvSource {
  type: "csv";
  /** 文件路径（相对于项目目录） */
  path: string;
  /** 分隔符 */
  delimiter?: string;
  /** 编码 */
  encoding?: string;
  /** 是否有表头 */
  has_header?: boolean;
}

/** JSON 数据源 */
export interface JsonSource {
  type: "json";
  /** 文件路径 */
  path: string;
}

/** Excel 数据源 */
export interface ExcelSource {
  type: "excel";
  /** 文件路径 */
  path: string;
  /** 工作表名称 */
  sheet?: string;
  /** 起始行 */
  start_row?: number;
}

/** SQL 数据源 */
export interface SqlSource {
  type: "sql";
  /** 连接标识符 */
  connection_id: string;
  /** SQL 查询语句 */
  query: string;
  /** 查询参数 */
  parameters?: Record<string, unknown>;
}

/** API 数据源 */
export interface ApiSource {
  type: "api";
  /** 请求 URL */
  url: string;
  /** HTTP 方法 */
  method?: "GET" | "POST" | "PUT" | "DELETE";
  /** 请求头 */
  headers?: Record<string, string>;
  /** 请求体 */
  body?: unknown;
}

/** 转换数据源 */
export interface TransformSource {
  type: "transform";
  /** 源变量 ID */
  source_variable_id: string;
  /** 转换操作列表 */
  operations: TransformOperation[];
}

/** 内联数据源 */
export interface InlineSource {
  type: "inline";
  /** 内联数据 */
  data: unknown;
}

/** 数据来源配置 */
export type DataSourceConfig =
  | CsvSource
  | JsonSource
  | ExcelSource
  | SqlSource
  | ApiSource
  | TransformSource
  | InlineSource;

// ==================== 转换操作 ====================

/** 过滤操作 */
export interface FilterOperation {
  op: "filter";
  expression: string;
}

/** 选择列操作 */
export interface SelectOperation {
  op: "select";
  columns: string[];
}

/** 排序操作 */
export interface SortOperation {
  op: "sort";
  column: string;
  descending?: boolean;
}

/** 分组聚合操作 */
export interface GroupByOperation {
  op: "group_by";
  columns: string[];
  aggregations: Aggregation[];
}

/** 限制行数操作 */
export interface LimitOperation {
  op: "limit";
  count: number;
}

/** 表达式操作 */
export interface ExpressionOperation {
  op: "expression";
  expr: string;
}

/** 转换操作 */
export type TransformOperation =
  | FilterOperation
  | SelectOperation
  | SortOperation
  | GroupByOperation
  | LimitOperation
  | ExpressionOperation;

/** 聚合函数 */
export type AggregateFunction =
  | "sum"
  | "avg"
  | "min"
  | "max"
  | "count"
  | "first"
  | "last";

/** 聚合操作 */
export interface Aggregation {
  /** 源列 */
  column: string;
  /** 聚合函数 */
  function: AggregateFunction;
  /** 结果别名 */
  alias?: string;
}

// ==================== 变量定义 ====================

/**
 * 变量定义（持久化到项目文件）
 */
export interface VariableDefinition {
  // ===== 元数据 =====
  /** 变量 ID */
  id: string;
  /** 变量名称 */
  name: string;
  /** 数据类型 */
  data_type: VariableDataType;
  /** 描述 */
  description: string;

  // ===== 作用域 =====
  /** 变量作用域 */
  scope: VariableScope;

  // ===== 值配置 =====
  /** 变量值（简单类型） */
  static_value?: unknown;
  /** 数据来源配置（复杂类型） */
  source_config?: DataSourceConfig;

  // ===== 可选配置 =====
  /** 是否为常量 */
  is_constant?: boolean;
  /** 默认值 */
  default_value?: unknown;
  /** 是否暴露给外部 */
  is_exposed?: boolean;
  /** 标签 */
  tags?: string[];
}

// ==================== 运行时状态 ====================

/**
 * 变量运行时状态（不保存到项目文件）
 */
export interface VariableRuntimeState {
  /** 变量 ID */
  id: string;
  /** 是否已加载到内存 */
  isLoaded: boolean;
  /** 后端数据句柄 */
  handle?: string;
  /** UI 预览数据 */
  preview?: VariablePreview;
  /** 加载错误 */
  error?: string;
  /** 是否正在加载 */
  isLoading?: boolean;
}

/** 变量预览类型 */
export type VariablePreviewType = "value" | "table" | "summary" | "chart";

/** 变量预览 */
export interface VariablePreview {
  /** 预览类型 */
  type: VariablePreviewType;
  /** 预览数据 */
  data: unknown;
  /** 数据统计（用于 DataFrame） */
  stats?: DataFrameStats;
}

/** DataFrame 统计信息 */
export interface DataFrameStats {
  /** 行数 */
  row_count: number;
  /** 列数 */
  column_count: number;
  /** 列信息 */
  columns: ColumnInfo[];
}

/** 列信息 */
export interface ColumnInfo {
  /** 列名 */
  name: string;
  /** 数据类型 */
  dtype: string;
  /** 非空值数量 */
  non_null_count: number;
  /** 唯一值数量 */
  unique_count?: number;
}

// ==================== 辅助函数 ====================

/**
 * 检查变量是否为简单类型
 */
export function isPrimitiveType(dataType: VariableDataType): boolean {
  return ["int", "float", "bool", "string"].includes(dataType);
}

/**
 * 检查变量是否为复杂类型
 */
export function isComplexType(dataType: VariableDataType): boolean {
  return ["dataframe", "object", "array"].includes(dataType);
}

/**
 * 获取数据类型的显示名称
 */
export function getDataTypeDisplayName(dataType: VariableDataType): string {
  const displayNames: Record<VariableDataType, string> = {
    int: "整数",
    float: "浮点数",
    bool: "布尔",
    string: "字符串",
    object: "对象",
    array: "数组",
    dataframe: "数据框",
    any: "任意",
  };
  return displayNames[dataType] ?? dataType;
}

/**
 * 获取数据类型的默认值
 */
export function getDefaultValueForType(dataType: VariableDataType): unknown {
  const defaults: Record<VariableDataType, unknown> = {
    int: 0,
    float: 0.0,
    bool: false,
    string: "",
    object: {},
    array: [],
    dataframe: null,
    any: null,
  };
  return defaults[dataType];
}

/**
 * 创建新的变量定义
 */
export function createVariableDefinition(
  id: string,
  name: string,
  dataType: VariableDataType,
  scope: VariableScope = { type: "global" }
): VariableDefinition {
  const defaultValue = getDefaultValueForType(dataType);
  return {
    id,
    name,
    data_type: dataType,
    description: "",
    scope,
    static_value: defaultValue,
    default_value: defaultValue,
    is_constant: false,
    is_exposed: false,
    tags: [],
  };
}

/**
 * 创建简单类型变量
 */
export function createPrimitiveVariable(
  id: string,
  name: string,
  dataType: VariableDataType,
  value: unknown
): VariableDefinition {
  const variable = createVariableDefinition(id, name, dataType);
  variable.static_value = value;
  variable.default_value = value;
  return variable;
}

/**
 * 创建复杂类型变量
 */
export function createComplexVariable(
  id: string,
  name: string,
  dataType: VariableDataType,
  source: DataSourceConfig
): VariableDefinition {
  const variable = createVariableDefinition(id, name, dataType);
  variable.source_config = source;
  return variable;
}
