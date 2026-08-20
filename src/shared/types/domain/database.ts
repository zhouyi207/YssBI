/**
 * Domain Types - Database
 * 
 * 数据库和数据源相关的类型定义
 */

// ==================== 数据库声明 ====================

export type {
  ColumnInfo,
  DatabaseDeclDTO,
  DatabaseDeclDTO as DatabaseDecl,
  DatabaseEngineDTO,
  DatabaseImportSourceDTO,
  DatabaseRecord,
} from '../dto/database';

export {
  databaseRecordFromLoad,
  databaseSourcePath,
  displayNameFromEngine,
  normalizeDatabaseRecord,
  normalizeDatabases,
} from '../dto/database';

// ==================== 数据来源配置 ====================

/**
 * CSV 数据源
 */
export interface CsvSource {
    type: "csv";
    path: string;           // 文件路径（相对于项目目录）
    delimiter?: string;     // 分隔符
    encoding?: string;      // 编码
    has_header?: boolean;   // 是否有表头
}

/**
 * JSON 数据源
 */
export interface JsonSource {
    type: "json";
    path: string;           // 文件路径
}

/**
 * Excel 数据源
 */
export interface ExcelSource {
    type: "excel";
    path: string;           // 文件路径
    sheet?: string;         // 工作表名称
    start_row?: number;     // 起始行
}

/**
 * SQL 数据源
 */
export interface SqlSource {
    type: "sql";
    connection_id: string;              // 连接标识符
    query: string;                      // SQL 查询语句
    parameters?: Record<string, unknown>; // 查询参数
}

/**
 * API 数据源
 */
export interface ApiSource {
    type: "api";
    url: string;                        // 请求 URL
    method?: "GET" | "POST" | "PUT" | "DELETE"; // HTTP 方法
    headers?: Record<string, string>;   // 请求头
    body?: unknown;                     // 请求体
}

/**
 * 转换数据源
 */
export interface TransformSource {
    type: "transform";
    source_variable_id: string;         // 源变量 ID
    operations: TransformOperation[];   // 转换操作列表
}

/**
 * 内联数据源
 */
export interface InlineSource {
    type: "inline";
    data: unknown;                      // 内联数据
}

/**
 * 数据来源配置
 * 支持多种数据源类型
 */
export type DataSourceConfig =
    | CsvSource
    | JsonSource
    | ExcelSource
    | SqlSource
    | ApiSource
    | TransformSource
    | InlineSource;

// ==================== 转换操作 ====================

/**
 * 过滤操作
 */
export interface FilterOperation {
    op: "filter";
    expression: string;
}

/**
 * 选择列操作
 */
export interface SelectOperation {
    op: "select";
    columns: string[];
}

/**
 * 排序操作
 */
export interface SortOperation {
    op: "sort";
    column: string;
    descending?: boolean;
}

/**
 * 分组聚合操作
 */
export interface GroupByOperation {
    op: "group_by";
    columns: string[];
    aggregations: Aggregation[];
}

/**
 * 限制行数操作
 */
export interface LimitOperation {
    op: "limit";
    count: number;
}

/**
 * 表达式操作
 */
export interface ExpressionOperation {
    op: "expression";
    expr: string;
}

/**
 * 转换操作
 * 用于数据转换和处理
 */
export type TransformOperation =
    | FilterOperation
    | SelectOperation
    | SortOperation
    | GroupByOperation
    | LimitOperation
    | ExpressionOperation;

/**
 * 聚合函数
 */
export type AggregateFunction =
    | "sum"
    | "avg"
    | "min"
    | "max"
    | "count"
    | "first"
    | "last";

/**
 * 聚合操作
 */
export interface Aggregation {
    column: string;         // 源列
    function: AggregateFunction; // 聚合函数
    alias?: string;         // 结果别名
}
