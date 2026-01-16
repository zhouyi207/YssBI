import { BaseNode } from "./models";
import { NODE_REGISTRY } from "./registry";

export function createNodeFromTemplate(
  pos: { x: number; y: number },
  _scale: number,
  type: string
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
    definition.title,
    pos,
    ...(definition.extraArgs || [])
  );

  node.inputs = definition.initialInputs?.map((pin) => ({
    id: `${nodeId}-input-${pin.id}`,
    name: pin.name,
    type: pin.type,
    direction: "input",
    connectedTo: [],
    defaultValue: pin.defaultValue,
  })) ?? [];

  node.outputs = definition.initialOutputs?.map((pin) => ({
    id: `${nodeId}-output-${pin.id}`,
    name: pin.name,
    type: pin.type,
    direction: "output",
    connectedTo: [],
    defaultValue: pin.defaultValue,
  })) ?? [];

  return node;
}
