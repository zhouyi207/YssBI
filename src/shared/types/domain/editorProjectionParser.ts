import {
  isEditorGraphProjectionDto,
  isParameterEditor,
} from "@/shared/types/domain/editorProjectionGuards";
import type {
  EditorGraphProjectionDto,
  PortAddressDto,
} from "@/shared/types/domain/editorProjection";

export function parseEditorGraphProjectionDto(value: unknown): EditorGraphProjectionDto {
  if (!isEditorGraphProjectionDto(value)) {
    throw new Error("Invalid editor graph projection response");
  }
  return validateEditorGraphProjection(value);
}

export function validateEditorGraphProjection(
  projection: EditorGraphProjectionDto,
): EditorGraphProjectionDto {
  if (projection.basis.graphPath !== projection.graphPath) {
    throw new Error(
      `projection basis graph path '${projection.basis.graphPath}' does not match projection graph path '${projection.graphPath}'`,
    );
  }
  const nodeIds = new Set<string>();
  const portDirections = new Map<string, "input" | "output">();
  for (const node of projection.nodes) {
    validateNode(projection, nodeIds, portDirections, node);
  }

  const connectionIds = new Set<string>();
  for (const connection of projection.connections) {
    if (connectionIds.has(connection.connectionId)) {
      throw new Error(`projection contains duplicate connection '${connection.connectionId}'`);
    }
    connectionIds.add(connection.connectionId);

    const outputDirection = portDirections.get(portAddressKey(connection.output));
    const inputDirection = portDirections.get(portAddressKey(connection.input));
    if (!outputDirection || !inputDirection) {
      throw new Error(
        `projection connection '${connection.connectionId}' references a missing port`,
      );
    }
    if (outputDirection !== "output" || inputDirection !== "input") {
      if (
        !projection.diagnostics.some(
          (diagnostic) =>
            diagnostic.blocking &&
            diagnostic.location.kind === "connection" &&
            diagnostic.location.connectionId === connection.connectionId,
        )
      )
        throw new Error(
          `projection connection '${connection.connectionId}' endpoint direction is invalid`,
        );
    }
  }

  return projection;
}

function validateNode(
  projection: EditorGraphProjectionDto,
  nodeIds: Set<string>,
  portDirections: Map<string, "input" | "output">,
  node: EditorGraphProjectionDto["nodes"][number],
): void {
  if (nodeIds.has(node.nodeId)) {
    throw new Error(`projection contains duplicate node '${node.nodeId}'`);
  }
  nodeIds.add(node.nodeId);

  if (node.graphPath !== projection.graphPath) {
    throw new Error(
      `projection node '${node.nodeId}' graph path '${node.graphPath}' does not match projection graph path '${projection.graphPath}'`,
    );
  }
  const groupKeys = new Set<string>();
  const parameterKeys = new Set<string>();
  for (const group of node.parameterGroups) {
    if (groupKeys.has(group.key)) throw new Error(`duplicate parameter group '${group.key}'`);
    groupKeys.add(group.key);
    for (const parameter of group.parameters) {
      if (!isParameterEditor(parameter) || parameterKeys.has(parameter.key)) {
        throw new Error(`projection parameter editor '${parameter.key}' is invalid or duplicated`);
      }
      parameterKeys.add(parameter.key);
    }
  }

  for (const port of node.ports) {
    if (port.address.nodeId !== node.nodeId) {
      throw new Error(
        `projection port is owned by node '${port.address.nodeId}' but is contained by node '${node.nodeId}'`,
      );
    }

    const key = portAddressKey(port.address);
    if (portDirections.has(key)) {
      throw new Error(`projection contains duplicate port '${key}'`);
    }
    portDirections.set(key, port.direction);
  }

  const additionKeys = new Set<string>();
  for (const addition of node.portInstanceAdditions) {
    if (additionKeys.has(addition.templateKey)) {
      throw new Error(
        `projection node '${node.nodeId}' contains duplicate port instance addition '${addition.templateKey}'`,
      );
    }
    additionKeys.add(addition.templateKey);
  }
}

export function portAddressKey(address: PortAddressDto): string {
  return address.kind === "declared"
    ? JSON.stringify(["declared", address.nodeId, address.portKey])
    : JSON.stringify(["instance", address.nodeId, address.templateKey, address.instanceId]);
}
