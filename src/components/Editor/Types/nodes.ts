/**
 * 节点类型定义
 *
 * 注意：PinType 现在从后端 schema 获取，这里保留类型定义用于 TypeScript 类型检查。
 * 实际的类型元数据（颜色、转换规则等）请使用 useSchemaStore。
 */

import { Position } from "../../../types";

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
  category: string;
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
}

export class BaseNode {
  id: string;
  type: string;
  title: string;
  category: string;
  position: Position;
  inputs: Pin[] = [];
  outputs: Pin[] = [];
  variableId?: string; // 关联的变量 ID
  variableType?: string; // 关联的变量类型
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
      direction: "input",
      links: [],
      defaultValue: p.defaultValue
    }));

    this.outputs = definition.outputs.map((p, i) => ({
      id: `${id}_out_${i}`,
      nodeId: id,
      name: p.name,
      type: p.type,
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
}
