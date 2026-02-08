/**
 * Schema 类型定义
 *
 * 这些类型与后端的 schema 模块对应。
 * 前端启动时从后端获取这些定义并缓存。
 */

// ==================== Pin 类型 ====================

/** Pin 类型定义 */
export interface PinTypeDefinition {
  /** 类型标识符 (如 "exec", "float", "int", "string", "bool") */
  name: string;
  /** 显示名称 */
  display_name: string;
  /** 是否为执行类型 */
  is_exec: boolean;
  /** 是否支持数组模式 */
  supports_array: boolean;
  /** 可以隐式转换到的类型列表 */
  implicit_convert_to: string[];
  /** 可以显式转换到的类型列表 */
  explicit_convert_to: string[];
  /** 默认值 */
  default_value: any | null;
}

/** 类型转换结果 */
export type TypeConversion = "Same" | "Implicit" | "Explicit" | "Incompatible";

// ==================== 分类 ====================

/** 节点分类定义 */
export interface CategoryDefinition {
  /** 分类标识符 */
  name: string;
  /** 显示名称 */
  display_name: string;
  /** 分类描述 */
  description: string | null;
  /** 排序权重 (越小越靠前) */
  sort_order: number;
  /** 分类颜色 */
  color: string | null;
  /** 图标名称 */
  icon: string | null;
  /** 是否在节点面板中显示 */
  visible_in_palette: boolean;
}

// ==================== UI 样式 ====================

/** UI 样式定义 */
export interface UIStyleDefinition {
  /** 样式标识符 */
  name: string;
  /** 显示名称 */
  display_name: string;
  /** 是否显示标题栏 */
  has_header: boolean;
  /** 是否为紧凑模式 */
  compact: boolean;
  /** 标题栏背景颜色 */
  header_color: string | null;
  /** 节点背景颜色 */
  background_color: string | null;
  /** 最小宽度 */
  min_width: number | null;
  /** 最小高度 */
  min_height: number | null;
  /** 中心符号映射 (node_type -> symbol) */
  center_symbols: Record<string, string>;
}

// ==================== 变量类型 ====================

/** 编辑器控件类型 */
export type EditorWidget =
  | { type: "Number"; config: { min?: number; max?: number; step?: number; precision?: number } }
  | { type: "Text"; config: { multiline: boolean; max_length?: number; placeholder?: string } }
  | { type: "Checkbox" }
  | { type: "Select"; config: { options: { value: any; label: string }[] } }
  | { type: "Color" }
  | { type: "JsonEditor" }
  | { type: "ArrayEditor"; config: { item_type: string } };

/** 变量类型定义 */
export interface VariableTypeDefinition {
  /** 类型标识符 */
  name: string;
  /** 显示名称 */
  display_name: string;
  /** 对应的 Pin 类型 */
  pin_type: string;
  /** 默认值 */
  default_value: any;
  /** 编辑器控件类型 */
  editor_widget: EditorWidget;
  /** 是否支持数组 */
  supports_array: boolean;
}

// ==================== 验证规则 ====================

/** 验证级别 */
export type ValidationLevel = "Error" | "Warning" | "Info";

/** Pin 验证规则 */
export interface PinValidationRule {
  pin_name: string;
  required: boolean;
  min_connections: number;
  max_connections: number | null;
}

/** 节点验证规则 */
export interface NodeValidationRule {
  node_type: string;
  input_rules: PinValidationRule[];
  output_rules: PinValidationRule[];
  custom_message: string | null;
}

/** 图规则类型 */
export type GraphRuleType =
  | { type: "RequireEntryNode"; entry_node_types: string[] }
  | { type: "NoCycles" }
  | { type: "AllPathsTerminate" }
  | { type: "NoUnusedOutputs"; excluded_types: string[] };

/** 图验证规则 */
export interface GraphValidationRule {
  name: string;
  description: string;
  level: ValidationLevel;
  rule_type: GraphRuleType;
}

/** 验证消息 */
export interface ValidationMessage {
  level: ValidationLevel;
  message: string;
  node_id: string | null;
  pin_id: string | null;
}

/** 验证结果 */
export interface ValidationResult {
  valid: boolean;
  messages: ValidationMessage[];
}

// ==================== 完整 Schema ====================

/** 完整的编辑器 Schema */
export interface EditorSchema {
  pin_types: PinTypeDefinition[];
  categories: CategoryDefinition[];
  ui_styles: UIStyleDefinition[];
  variable_types: VariableTypeDefinition[];
  node_validation_rules: NodeValidationRule[];
  graph_validation_rules: GraphValidationRule[];
}

// ==================== 重新导出变量定义类型 ====================

export type {
  VariableDataType,
  VariableScope,
  GlobalScope,
  FunctionScope,
  MacroScope,
  DataSourceConfig,
  CsvSource,
  JsonSource,
  ExcelSource,
  SqlSource,
  ApiSource,
  TransformSource,
  InlineSource,
  TransformOperation,
  Aggregation,
  AggregateFunction,
  VariableDefinition,
  VariableRuntimeState,
  VariablePreview,
  VariablePreviewType,
  DataFrameStats,
  ColumnInfo,
} from "./variables";

export {
  isPrimitiveType,
  isComplexType,
  getDataTypeDisplayName,
  getDefaultValueForType,
  createVariableDefinition,
  createPrimitiveVariable,
  createComplexVariable,
} from "./variables";
