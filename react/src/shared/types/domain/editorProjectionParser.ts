import {
  isEditorGraphProjectionDto,
  hasValidatedEditorNode,
  isParameterEditor,
} from "@/shared/types/domain/editorProjectionGuards";
import { isPublishedValue } from "@/shared/types/deepReadonly";
import { portAddressKey } from "@/shared/types/domain/portAddressKey";
import type { EditorGraphProjectionDto } from "@/shared/types/domain/editorProjection";

type NodeProjection = EditorGraphProjectionDto["nodes"][number];
const validatedNodePortKeys = new WeakMap<NodeProjection, readonly string[]>();
const validatedProjections = new WeakSet<EditorGraphProjectionDto>();

export function parseEditorGraphProjectionDto(value: unknown): EditorGraphProjectionDto {
  if (!isEditorGraphProjectionDto(value)) {
    throw new Error("Invalid editor graph projection response");
  }
  return validateEditorGraphProjection(value);
}

export function validateEditorGraphProjection(
  projection: EditorGraphProjectionDto,
): EditorGraphProjectionDto {
  if (validatedProjections.has(projection)) return projection;
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
  let blockedConnections: Set<string> | undefined;
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
      if (!blockedConnections) {
        blockedConnections = new Set();
        for (const diagnostic of projection.diagnostics) {
          if (diagnostic.blocking && diagnostic.location.kind === "connection")
            blockedConnections.add(diagnostic.location.connectionId);
        }
      }
      if (!blockedConnections.has(connection.connectionId))
        throw new Error(
          `projection connection '${connection.connectionId}' endpoint direction is invalid`,
        );
    }
  }

  if (isPublishedValue(projection)) validatedProjections.add(projection);
  return projection;
}

function validateNode(
  projection: EditorGraphProjectionDto,
  nodeIds: Set<string>,
  portDirections: Map<string, "input" | "output">,
  node: NodeProjection,
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
  const cachedKeys = validatedNodePortKeys.get(node);
  let portKeys = cachedKeys;
  if (!portKeys) {
    // The shape guard also owns parameter metadata and group/key uniqueness. Reuse its
    // immutable-node proof instead of building the same parameter sets a second time.
    if (!hasValidatedEditorNode(node)) {
      const groupKeys = new Set<string>();
      const parameterKeys = new Set<string>();
      for (const group of node.parameterGroups) {
        if (groupKeys.has(group.key)) throw new Error(`duplicate parameter group '${group.key}'`);
        groupKeys.add(group.key);
        for (const parameter of group.parameters) {
          if (!isParameterEditor(parameter) || parameterKeys.has(parameter.key)) {
            throw new Error(
              `projection parameter editor '${parameter.key}' is invalid or duplicated`,
            );
          }
          parameterKeys.add(parameter.key);
        }
      }
    }
    portKeys = node.ports.map((port) => {
      if (port.address.nodeId !== node.nodeId) {
        throw new Error(
          `projection port is owned by node '${port.address.nodeId}' but is contained by node '${node.nodeId}'`,
        );
      }
      return portAddressKey(port.address);
    });
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
  // Cache only node-local checks. Membership, graph identity and endpoint directions are
  // checked against this candidate even when all of its node objects were validated before.
  for (let index = 0; index < portKeys.length; index++) {
    const key = portKeys[index];
    if (portDirections.has(key)) {
      throw new Error(`projection contains duplicate port '${key}'`);
    }
    portDirections.set(key, node.ports[index].direction);
  }
  if (!cachedKeys && isPublishedValue(node)) validatedNodePortKeys.set(node, portKeys);
}
