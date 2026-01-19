import { BaseNode } from "./models";
import { NODE_REGISTRY } from "./registry";

export function createNodeFromTemplate(
  pos: { x: number; y: number },
  _scale: number,
  type: string,
  initialProps?: Partial<BaseNode> & { variableType?: string }
): BaseNode | null {
  const definition = NODE_REGISTRY[type];
  if (!definition) {
    console.error(`Node type ${type} not found in registry`);
    return null;
  }

  const nodeId = `node-${crypto.randomUUID()}`;
  
  const node = new definition.className(
    nodeId,
    definition.type,
    initialProps?.title || definition.title,
    pos,
    ...(definition.extraArgs || [])
  );

  // Apply other initial props
  if (initialProps) {
    const { variableType, ...rest } = initialProps;
    Object.assign(node, rest);
  }

  node.inputs = definition.initialInputs?.map((pin) => {
    let pinType = pin.type;
    // 如果是变量相关节点且提供了变量类型，则覆盖默认的数据针脚类型
    if (initialProps?.variableType && (type === "get_variable" || type === "set_variable") && pin.type !== "exec") {
      pinType = initialProps.variableType as any;
    }
    return {
      id: `${nodeId}-input-${pin.id}`,
      nodeId,
      name: pin.name,
      type: pinType,
      direction: "input",
      links: [],
      defaultValue: pin.defaultValue,
    };
  }) ?? [];

  node.outputs = definition.initialOutputs?.map((pin) => {
    let pinType = pin.type;
    if (initialProps?.variableType && (type === "get_variable" || type === "set_variable") && pin.type !== "exec") {
      pinType = initialProps.variableType as any;
    }
    return {
      id: `${nodeId}-output-${pin.id}`,
      nodeId,
      name: pin.name,
      type: pinType,
      direction: "output",
      links: [],
      defaultValue: pin.defaultValue,
    };
  }) ?? [];

  return node;
}
