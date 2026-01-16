import { MathNode, EventNode, BranchNode, VariableNode, BaseNode, NodeType, PinType } from "./models";
import { Position } from "../../types";

export interface PinDefinition {
  id: string;
  name: string;
  type: PinType;
  defaultValue?: any;
}

export interface NodeDefinition {
  type: string;
  category: NodeType;
  title: string;
  className: new (id: string, type: string, title: string, pos: Position, ...args: any[]) => BaseNode;
  extraArgs?: any[];
  initialInputs?: PinDefinition[];
  initialOutputs?: PinDefinition[];
  ui?: {
    icon?: string;
    color?: string;
  };
}

export const NODE_REGISTRY: Record<string, NodeDefinition> = {
  "get_variable": {
    type: "get_variable",
    category: "Variable",
    title: "Get Variable",
    className: VariableNode,
    initialOutputs: [{ id: "val", name: "Value", type: "int" }]
  },
  "set_variable": {
    type: "set_variable",
    category: "Variable",
    title: "Set Variable",
    className: VariableNode,
    initialInputs: [{ id: "val", name: "Value", type: "int" }],
    initialOutputs: [{ id: "val", name: "Value", type: "int" }]
  },
  "add": {
    type: "add",
    category: "Math",
    title: "Add",
    className: MathNode,
    extraArgs: ["+"],
    initialInputs: [
      { id: "a", name: "A", type: "int" }, 
      { id: "b", name: "B", type: "int" }
    ],
    initialOutputs: [{ id: "sum", name: "Sum", type: "int" }],
    ui: { icon: "∑" }
  },
  "on_start": {
    type: "on_start",
    category: "Event",
    title: "On Start",
    className: EventNode,
    initialOutputs: [{ id: "exec", name: "Out", type: "exec" }],
    ui: { icon: "🚀" }
  },
  "if_else": {
    type: "if_else",
    category: "Branch",
    title: "Branch",
    className: BranchNode,
    initialInputs: [
      { id: "exec_in", name: "In", type: "exec" }, 
      { id: "condition", name: "Cond", type: "bool" }
    ],
    initialOutputs: [
      { id: "true", name: "True", type: "exec" }, 
      { id: "false", name: "False", type: "exec" }
    ],
    ui: { icon: "⑂" }
  }
};
