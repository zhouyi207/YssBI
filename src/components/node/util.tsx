import { BaseNode } from "./models";
import { NODE_REGISTRY } from "./registry";

export function createNodeFromTemplate(
  pos: { x: number; y: number },
  _scale: number,
  type: string,
  initialProps?: Partial<BaseNode> & { variableType?: string }
): BaseNode | null {
  const nodeId = `node-${crypto.randomUUID()}`;
  const node = NODE_REGISTRY.createNode(type, nodeId, pos);
  
  if (!node) return null;

  // Apply other initial props
  if (initialProps) {
    const { variableType, title, ...rest } = initialProps;
    if (title) node.title = title;
    Object.assign(node, rest);

    // 如果是变量相关节点且提供了变量类型，则动态更新针脚类型
    if (variableType && (type === "get_variable" || type === "set_variable")) {
      const updatePins = (pins: any[]) => {
        pins.forEach(p => {
          if (p.type !== "exec") {
            p.type = variableType;
          }
        });
      };
      updatePins(node.inputs);
      updatePins(node.outputs);
    }
  }

  return node;
}
