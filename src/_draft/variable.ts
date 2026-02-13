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


/**
 * 获取数据类型的默认值
 */
export function getDefaultValueForType(dataType: DataType): unknown {
  const defaults: Record<DataType, unknown> = {
    int8: 0,
    int16: 0,
    int32: 0,
    int64: 0,
    uint32: 0,
    uint64: 0,
    float32: 0.0,
    float64: 0.0,
    bool: false,
    string: "",
    date: null,
    datetime: null,
    object: {},
    array: [],
    dataframe: null,
  };
  return defaults[dataType];
}

/**
 * 创建新的变量定义
 */
export function createVariableDefinition(
  id: string,
  name: string,
  dataType: DataType,
  scope: VariableScope = { type: "global" }
): Variable {
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
  dataType: DataType,
  value: unknown
): Variable {
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
  dataType: DataType,
): Variable {
  const variable = createVariableDefinition(id, name, dataType);
  return variable;
}


export function isPrimitiveType(dataType: DataType): boolean {
  return [
    "int", "int8", "int16", "int32", "int64",
    "uint32", "uint64",
    "float", "float32", "float64",
    "bool", "string", "date", "datetime"
  ].includes(dataType);
}

/**
 * 检查变量是否为复杂类型
 */
export function isComplexType(dataType: DataType): boolean {
  return ["dataframe", "object", "array"].includes(dataType);
}


