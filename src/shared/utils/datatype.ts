import { DataType } from "@/shared/types/editor";

/**
 * 检查数据类型是否为基础类型（可以直接编辑值）
 */
export function isPrimitiveType(dataType: DataType): boolean {
  return [
    // 新格式（大写开头）
    "Boolean", "Int32", "Int64", "Float32", "Float64", "String",
    // 旧格式（小写，保留兼容）
    "int", "int8", "int16", "int32", "int64",
    "uint32", "uint64",
    "float", "float32", "float64",
    "bool", "string", "date", "datetime"
  ].includes(dataType as string);
}

/**
 * 检查数据类型是否为复杂类型
 */
export function isComplexType(dataType: DataType): boolean {
  return [
    "DataFrame", "Object", "Array", 
    "dataframe", "object", "array"
  ].includes(dataType as string);
}

/**
 * 获取数据类型的默认值
 */
export function getDefaultValue(dataType: DataType | string): unknown {
  const typeStr = dataType as string;
  switch (typeStr) {
    case "Boolean":
    case "bool":
      return false;
    case "Int32":
    case "Int64":
    case "int":
    case "int8":
    case "int16":
    case "int32":
    case "int64":
    case "uint32":
    case "uint64":
      return 0;
    case "Float32":
    case "Float64":
    case "float":
    case "float32":
    case "float64":
      return 0.0;
    case "String":
    case "string":
      return "";
    case "Array":
    case "array":
      return [];
    case "Object":
    case "object":
      return {};
    default:
      return undefined;
  }
}

/**
 * 将旧格式的类型名转换为新格式
 */
export function normalizeDataType(dataType: string): DataType {
  const typeMap: Record<string, DataType> = {
    "bool": "Boolean",
    "int": "Int32",
    "int8": "Int32",
    "int16": "Int32",
    "int32": "Int32",
    "int64": "Int64",
    "uint32": "Int32",
    "uint64": "Int64",
    "float": "Float32",
    "float32": "Float32",
    "float64": "Float64",
    "string": "String",
    "object": "Object",
    "array": "Array",
    "dataframe": "DataFrame",
  };
  
  return (typeMap[dataType.toLowerCase()] || dataType) as DataType;
}
