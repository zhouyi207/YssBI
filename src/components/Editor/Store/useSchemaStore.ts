/**
 * Schema Store
 *
 * 管理从后端获取的所有 schema 定义。
 * 这是前端的 schema 缓存，作为类型元数据的单一数据源。
 */

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type {
  EditorSchema,
  PinTypeDefinition,
  CategoryDefinition,
  UIStyleDefinition,
  VariableTypeDefinition,
  NodeValidationRule,
  GraphValidationRule,
} from "../Types/schema";

interface SchemaStore {
  // 状态
  isLoaded: boolean;
  isLoading: boolean;
  error: string | null;

  // Schema 数据
  pinTypes: Map<string, PinTypeDefinition>;
  categories: Map<string, CategoryDefinition>;
  uiStyles: Map<string, UIStyleDefinition>;
  variableTypes: Map<string, VariableTypeDefinition>;
  nodeValidationRules: Map<string, NodeValidationRule>;
  graphValidationRules: GraphValidationRule[];

  // 操作
  loadSchema: () => Promise<void>;

  // 查询方法
  getPinType: (name: string) => PinTypeDefinition | undefined;
  getCategory: (name: string) => CategoryDefinition | undefined;
  getUIStyle: (name: string) => UIStyleDefinition | undefined;
  getVariableType: (name: string) => VariableTypeDefinition | undefined;
  getNodeValidationRule: (nodeType: string) => NodeValidationRule | undefined;

  // 类型兼容性
  canConnect: (fromType: string, toType: string) => boolean;
  getCenterSymbol: (styleName: string, nodeType: string) => string | undefined;

  // 列表获取
  getVisibleCategories: () => CategoryDefinition[];
  getAllPinTypes: () => PinTypeDefinition[];
  getAllVariableTypes: () => VariableTypeDefinition[];
}

export const useSchemaStore = create<SchemaStore>((set, get) => ({
  // 初始状态
  isLoaded: false,
  isLoading: false,
  error: null,

  pinTypes: new Map(),
  categories: new Map(),
  uiStyles: new Map(),
  variableTypes: new Map(),
  nodeValidationRules: new Map(),
  graphValidationRules: [],

  // 加载 schema
  loadSchema: async () => {
    if (get().isLoaded || get().isLoading) return;

    set({ isLoading: true, error: null });

    try {
      const schema: EditorSchema = await invoke("get_editor_schema_command");

      // 转换为 Map 以便快速查找
      const pinTypes = new Map<string, PinTypeDefinition>();
      schema.pin_types.forEach((pt) => pinTypes.set(pt.name, pt));

      const categories = new Map<string, CategoryDefinition>();
      schema.categories.forEach((cat) => categories.set(cat.name, cat));

      const uiStyles = new Map<string, UIStyleDefinition>();
      schema.ui_styles.forEach((style) => uiStyles.set(style.name, style));

      const variableTypes = new Map<string, VariableTypeDefinition>();
      schema.variable_types.forEach((vt) => variableTypes.set(vt.name, vt));

      const nodeValidationRules = new Map<string, NodeValidationRule>();
      schema.node_validation_rules.forEach((rule) =>
        nodeValidationRules.set(rule.node_type, rule)
      );

      set({
        isLoaded: true,
        isLoading: false,
        pinTypes,
        categories,
        uiStyles,
        variableTypes,
        nodeValidationRules,
        graphValidationRules: schema.graph_validation_rules,
      });

      console.log("[SchemaStore] Schema loaded successfully", {
        pinTypes: pinTypes.size,
        categories: categories.size,
        uiStyles: uiStyles.size,
        variableTypes: variableTypes.size,
      });
    } catch (err) {
      console.error("[SchemaStore] Failed to load schema:", err);
      set({
        isLoading: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  // 查询方法
  getPinType: (name) => get().pinTypes.get(name),
  getCategory: (name) => get().categories.get(name),
  getUIStyle: (name) => get().uiStyles.get(name),
  getVariableType: (name) => get().variableTypes.get(name),
  getNodeValidationRule: (nodeType) => get().nodeValidationRules.get(nodeType),

  // 类型兼容性检查
  canConnect: (fromType, toType) => {
    // 相同类型
    if (fromType === toType) return true;

    // object 可以接受任何非 exec 类型
    if (toType === "object" && fromType !== "exec") return true;

    // 检查隐式转换
    const fromDef = get().pinTypes.get(fromType);
    if (fromDef && fromDef.implicit_convert_to.includes(toType)) {
      return true;
    }

    return false;
  },

  // 获取中心符号
  getCenterSymbol: (styleName, nodeType) => {
    const style = get().uiStyles.get(styleName);
    return style?.center_symbols[nodeType];
  },

  // 获取可见分类（按 sort_order 排序）
  getVisibleCategories: () => {
    return Array.from(get().categories.values())
      .filter((cat) => cat.visible_in_palette)
      .sort((a, b) => a.sort_order - b.sort_order);
  },

  // 获取所有 Pin 类型
  getAllPinTypes: () => Array.from(get().pinTypes.values()),

  // 获取所有变量类型
  getAllVariableTypes: () => Array.from(get().variableTypes.values()),
}));

// 选择器 hooks
export const useSchemaLoaded = () => useSchemaStore((s) => s.isLoaded);
export const useSchemaLoading = () => useSchemaStore((s) => s.isLoading);
export const useSchemaError = () => useSchemaStore((s) => s.error);
export const useCanConnect = () => useSchemaStore((s) => s.canConnect);
