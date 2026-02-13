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