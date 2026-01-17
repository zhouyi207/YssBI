import { Position } from "../../types";

export type NodeType = 
  | "Event" 
  | "Function" 
  | "Variable" 
  | "Math" 
  | "Branch" 
  | "Custom";

export type PinDirection = "input" | "output";

export type PinType = 
  | "exec"      // 执行针脚
  | "int"       // 整数
  | "float"     // 浮点数
  | "string"    // 字符串
  | "bool"      // 布尔
  | "object"    // 对象
  | "array"     // 数组
  | "struct"    // 结构体
  | "delegate"; // 委托/事件

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
    color?: string;     // 可选，按类型渲染
  };
}

export abstract class BaseNode {
  id: string;
  type: string;
  title: string;
  position: Position;
  inputs: Pin[] = [];
  outputs: Pin[] = [];
  selected: boolean = false;
  
  // UI 标志位
  noHeader: boolean = false;
  centerSymbol?: string;

  constructor(id: string, type: string, title: string, position: Position) {
    this.id = id;
    this.type = type;
    this.title = title;
    this.position = position;
  }

  abstract get category(): NodeType;
  
  abstract execute(inputs: Record<string, any>, properties?: Record<string, any>): any;

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

  // 新增：轻量级克隆，仅用于拖拽位置更新，不克隆针脚
  cloneWithPosition(newPos: Position): this {
    const clone = Object.assign(Object.create(Object.getPrototypeOf(this)), this);
    clone.position = newPos;
    return clone;
  }
}

// --- Math 节点类 ---
export class MathNode extends BaseNode {
  centerSymbol: string = "";
  
  constructor(id: string, type: string, title: string, position: Position, symbol: string) {
    super(id, type, title, position);
    this.noHeader = true; // Math 节点默认不显示 Header
    this.centerSymbol = symbol;
  }

  get category(): NodeType { return "Math"; }

  execute(inputs: Record<string, any>) {
    return Object.values(inputs).reduce((a, b) => (Number(a) || 0) + (Number(b) || 0), 0);
  }
}

// --- Event 节点类 ---
export class EventNode extends BaseNode {
  get category(): NodeType { return "Event"; }
  execute() { /* 执行流入口 */ }
}

// --- Branch 节点类 ---
export class BranchNode extends BaseNode {
  get category(): NodeType { return "Branch"; }
  execute(inputs: Record<string, any>) {
    return !!inputs.condition;
  }
}

// --- Variable 节点类 ---
export class VariableNode extends BaseNode {
  get category(): NodeType { return "Variable"; }
  execute(_inputs: Record<string, any>, properties: Record<string, any>) {
    return properties.value;
  }
}
