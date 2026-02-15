/// store —— 只负责「状态 + backend 同步」

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { LoadStatus } from "@/shared/types/ui";
import {
  SchemaState,
  PinTypeDefinition,
  CategoryDefinition,
  UIStyleDefinition,
  VariableTypeDefinition,
  NodeValidationRule,
  GraphValidationRule,
  EditorSchema,
} from "../../../shared/types/domain/shema";

interface SchemaStore extends SchemaState {
  // Schema 数据
  pinTypes: Map<string, PinTypeDefinition>;
  categories: Map<string, CategoryDefinition>;
  uiStyles: Map<string, UIStyleDefinition>;
  variableTypes: Map<string, VariableTypeDefinition>;
  nodeValidationRules: Map<string, NodeValidationRule>;
  graphValidationRules: GraphValidationRule[];

  // 操作
  syncFromBackend: () => Promise<void>;
  clear: () => void;

  // 查询方法
  getPinType: (name: string) => PinTypeDefinition | undefined;
  getCategory: (name: string) => CategoryDefinition | undefined;
  getUIStyle: (name: string) => UIStyleDefinition | undefined;
  getVariableType: (name: string) => VariableTypeDefinition | undefined;
  getNodeValidationRule: (nodeType: string) => NodeValidationRule | undefined;

  // 列表获取
  getAllPinTypes: () => PinTypeDefinition[];
  getAllCategories: () => CategoryDefinition[];
  getAllUIStyles: () => UIStyleDefinition[];
  getAllVariableTypes: () => VariableTypeDefinition[];
  getVisibleCategories: () => CategoryDefinition[];
}

export const useSchemaStore = create<SchemaStore>((set, get) => ({
  // data
  pinTypes: new Map(),
  categories: new Map(),
  uiStyles: new Map(),
  variableTypes: new Map(),
  nodeValidationRules: new Map(),
  graphValidationRules: [],

  // state (来自 SchemaState)
  status: LoadStatus.Idle,
  error: null,

  syncFromBackend: async () => {
    const { status } = get();

    // 幂等保护
    if (status === LoadStatus.Loading || status === LoadStatus.Ready) {
      console.log('[Schema] Already loading or loaded, skipping...');
      return;
    }

    const startTime = performance.now();
    console.log('[Schema] Loading schema from backend...');

    set({ status: LoadStatus.Loading, error: null });

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
        pinTypes,
        categories,
        uiStyles,
        variableTypes,
        nodeValidationRules,
        graphValidationRules: schema.graph_validation_rules,
        status: LoadStatus.Ready,
      });

      const duration = performance.now() - startTime;
      console.log('[Schema] ✓ Schema loaded successfully', {
        pinTypes: pinTypes.size,
        categories: categories.size,
        uiStyles: uiStyles.size,
        variableTypes: variableTypes.size,
        validationRules: nodeValidationRules.size,
        duration: `${duration.toFixed(0)}ms`,
      });
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[Schema] ✗ Failed to load schema:', errorMessage);
      
      set({
        status: LoadStatus.Error,
        error: errorMessage,
      });
      
      throw err;
    }
  },

  clear: () =>
    set({
      pinTypes: new Map(),
      categories: new Map(),
      uiStyles: new Map(),
      variableTypes: new Map(),
      nodeValidationRules: new Map(),
      graphValidationRules: [],
      status: LoadStatus.Idle,
      error: null,
    }),

  // 查询方法
  getPinType: (name) => get().pinTypes.get(name),
  getCategory: (name) => get().categories.get(name),
  getUIStyle: (name) => get().uiStyles.get(name),
  getVariableType: (name) => get().variableTypes.get(name),
  getNodeValidationRule: (nodeType) => get().nodeValidationRules.get(nodeType),

  // 列表获取
  getAllPinTypes: () => Array.from(get().pinTypes.values()),
  getAllCategories: () => Array.from(get().categories.values()),
  getAllUIStyles: () => Array.from(get().uiStyles.values()),
  getAllVariableTypes: () => Array.from(get().variableTypes.values()),
  
  getVisibleCategories: () =>
    Array.from(get().categories.values())
      .filter((cat) => cat.visible_in_palette)
      .sort((a, b) => a.sort_order - b.sort_order),
}));
