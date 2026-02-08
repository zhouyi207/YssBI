/**
 * 节点类型定义
 *
 * 注意：PinType 现在从后端 schema 获取，这里保留类型定义用于 TypeScript 类型检查。
 * 实际的类型元数据（颜色、转换规则等）请使用 useSchemaStore。
 */

import { Position } from "../../../shared/types";

export type PinDirection = "input" | "output";

/**
 * Pin 类型标识符
 * 实际的类型定义（颜色、转换规则）从后端 schema 获取
 */
export type PinType =
  | "exec"      // 执行针脚
  | "int"       // 整数
  | "float"     // 浮点数
  | "string"    // 字符串
  | "bool"      // 布尔
  | "object"    // 对象
  | "array"     // 数组
  | "struct"    // 结构体
  | "delegate"  // 委托/事件
  | string;     // 允许自定义类型

export interface Pin {
  id: string;           // 唯一标识
  nodeId: string;       // 所属节点 ID
  name: string;         // 显示名称
  type: PinType;        // 类型
  direction: PinDirection;
  links: string[];      // 连接的 pin id
  defaultValue?: any;   // 默认值（如果是数据针脚）
  userValue?: any;      // 🆕 用户设置的值（覆盖默认值）
  isArray?: boolean;    // 是否为数组/List
  ui?: {
    x?: number;         // 在节点内部的位置
    y?: number;
    color?: string;     // 可选，按类型渲染（优先使用 schema 颜色）
  };
}

/**
 * 对应后端的 NodeDefinition
 */
export interface NodeDefinition {
  node_type: string;
  category: string[];
  title: string;
  inputs: PinDefinition[];
  outputs: PinDefinition[];
  ui_style: string;
  description?: string;
}

export interface PinDefinition {
  name: string;
  type: PinType;
  defaultValue?: any;
  isArray?: boolean;
}

export class BaseNode {
  id: string;
  type: string;
  title: string;
  category: string[];
  position: Position;
  inputs: Pin[] = [];
  outputs: Pin[] = [];
  // 变量节点相关字段 (用于 variable_get / variable_set 节点)
  variableId?: string;    // 关联的变量 ID
  variableType?: string;  // 关联的变量数据类型 (int, float, string, etc.)
  variableName?: string;  // 关联的变量名称 (用于显示)

  initialData?: any;      // 初始数据（用于特定节点类型）

  isInternal: boolean = false; // 是否为内部节点（不可删除）
  subGraphId?: string; // 关联的子图 ID (针对 Call Function / Call Macro)

  // UI 属性，由 NodeDefinition 驱动
  uiStyle: string = "default";
  centerSymbol?: string;

  constructor(id: string, definition: NodeDefinition, position: Position) {
    this.id = id;
    this.type = definition.node_type;
    this.title = definition.title;
    this.category = definition.category;
    this.position = position;
    this.uiStyle = definition.ui_style;

    // 根据定义初始化针脚
    this.inputs = definition.inputs.map((p, i) => ({
      id: `${id}_in_${i}`,
      nodeId: id,
      name: p.name,
      type: p.type,
      isArray: p.isArray,
      direction: "input",
      links: [],
      defaultValue: p.defaultValue
    }));

    this.outputs = definition.outputs.map((p, i) => ({
      id: `${id}_out_${i}`,
      nodeId: id,
      name: p.name,
      type: p.type,
      isArray: p.isArray,
      direction: "output",
      links: [],
      defaultValue: p.defaultValue
    }));

    // 注意：centerSymbol 现在应该从 schema 获取
    // 这里保留作为后备，实际使用请通过 useSchemaStore.getCenterSymbol()
    if (this.uiStyle === "math") {
      // 后备逻辑：如果 schema 未加载，使用硬编码
      const mathSymbols: Record<string, string> = {
        add: "+",
        subtract: "-",
        multiply: "×",
        divide: "÷",
      };
      this.centerSymbol = mathSymbols[this.type];
    }
  }

  get noHeader(): boolean {
    return this.uiStyle === "math";
  }

  // 通用的添加输入方法
  addInput(pin: Pin) {
    this.inputs.push(pin);
  }

  // 辅助方法：克隆对象（用于触发 React 更新）
  clone(): this {
    const clone = Object.assign(Object.create(Object.getPrototypeOf(this)), this);
    clone.inputs = this.inputs.map(p => ({ ...p, links: [...p.links] }));
    clone.outputs = this.outputs.map(p => ({ ...p, links: [...p.links] }));
    return clone;
  }

  cloneWithPosition(newPos: Position): this {
    const clone = Object.assign(Object.create(Object.getPrototypeOf(this)), this);
    clone.position = newPos;
    return clone;
  }

  /**
   * 更新变量节点的关联变量信息
   * 会同步更新节点的输入/输出 pin 类型
   */
  setVariable(variableId: string, variableName: string, variableType: string, isArray: boolean = false): this {
    this.variableId = variableId;
    this.variableName = variableName;
    this.variableType = variableType;

    // 更新节点标题显示变量名/数据帧名
    if (this.type === 'get_variable') {
      this.title = `Get ${variableName}`;
      // 更新输出 pin 的类型
      const valuePin = this.outputs.find(p => p.name === 'Value' || p.name === 'value');
      if (valuePin) {
        valuePin.type = variableType;
        valuePin.isArray = isArray;
      }
    } else if (this.type === 'get_dataframe') {
      this.title = `Get ${variableName}`;
      // 更新输出 pin 的类型
      const dfPin = this.outputs.find(p => p.name === 'DataFrame');
      if (dfPin) {
        dfPin.type = 'dataframe';
        dfPin.isArray = false; // DataFrame 本身通常不是数组 pin
      }
    } else if (this.type === 'set_variable') {
      this.title = `Set ${variableName}`;
      // 更新输入 pin 的类型
      const valuePin = this.inputs.find(p => p.name === 'Value' || p.name === 'value');
      if (valuePin) {
        valuePin.type = variableType;
        valuePin.isArray = isArray;
      }
      // 更新输出 pin 的类型 (pass-through)
      const outPin = this.outputs.find(p => p.name === 'Value' || p.name === 'value');
      if (outPin) {
        outPin.type = variableType;
        outPin.isArray = isArray;
      }
    }

    return this;
  }
}

/**
 * 创建变量 Get 节点
 */
export function createVariableGetNode(
  id: string,
  position: Position,
  variableId: string,
  variableName: string,
  variableType: string
): BaseNode {
  const definition: NodeDefinition = {
    node_type: 'get_variable',
    category: ['Variables'],
    title: `Get ${variableName}`,
    inputs: [],
    outputs: [{ name: 'Value', type: variableType }],
    ui_style: 'compact',
  };

  const node = new BaseNode(id, definition, position);
  node.variableId = variableId;
  node.variableName = variableName;
  node.variableType = variableType;

  return node;
}

/**
 * 创建变量 Set 节点
 */
export function createVariableSetNode(
  id: string,
  position: Position,
  variableId: string,
  variableName: string,
  variableType: string
): BaseNode {
  const definition: NodeDefinition = {
    node_type: 'set_variable',
    category: ['Variables'],
    title: `Set ${variableName}`,
    inputs: [
      { name: 'Exec', type: 'exec' },
      { name: 'Value', type: variableType },
    ],
    outputs: [
      { name: 'Then', type: 'exec' },
      { name: 'Value', type: variableType },
    ],
    ui_style: 'default',
  };

  const node = new BaseNode(id, definition, position);
  node.variableId = variableId;
  node.variableName = variableName;
  node.variableType = variableType;

  return node;
}
