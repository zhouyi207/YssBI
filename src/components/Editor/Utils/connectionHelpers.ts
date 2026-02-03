/**
 * Connection query helper functions
 * 
 * These functions provide convenient ways to query connections in a subgraph.
 * They work with the connections array (single source of truth) rather than Pin.links.
 */

import { Connection } from '../Types/canvas';
import { BaseNode } from '../Types/nodes';

/**
 * Find all connections that involve a specific pin
 * 
 * @param connections - Array of connections to search
 * @param pinId - ID of the pin to find connections for
 * @returns Array of connections where the pin is either source or target
 */
export function findConnectionsByPin(
  connections: Connection[],
  pinId: string
): Connection[] {
  return connections.filter(
    (conn) => conn.sourcePin === pinId || conn.targetPin === pinId
  );
}

/**
 * Find all connections that involve any pin on a specific node
 * 
 * @param connections - Array of connections to search
 * @param node - Node to find connections for
 * @returns Array of connections involving any pin on the node
 */
export function findConnectionsByNode(
  connections: Connection[],
  node: BaseNode
): Connection[] {
  // Get all pin IDs for this node
  const pinIds = new Set<string>();
  
  node.inputs.forEach((pin) => pinIds.add(pin.id));
  node.outputs.forEach((pin) => pinIds.add(pin.id));
  
  // Find connections involving any of these pins
  return connections.filter(
    (conn) => pinIds.has(conn.sourcePin) || pinIds.has(conn.targetPin)
  );
}

/**
 * Find a connection by its ID
 * 
 * @param connections - Array of connections to search
 * @param connectionId - ID of the connection to find
 * @returns The connection if found, null otherwise
 */
export function findConnectionById(
  connections: Connection[],
  connectionId: string
): Connection | null {
  return connections.find((conn) => conn.id === connectionId) || null;
}

/**
 * Check if two pins are connected
 * 
 * @param connections - Array of connections to search
 * @param pinId1 - ID of the first pin
 * @param pinId2 - ID of the second pin
 * @returns True if the pins are connected (in either direction)
 */
export function areConnected(
  connections: Connection[],
  pinId1: string,
  pinId2: string
): boolean {
  return connections.some(
    (conn) =>
      (conn.sourcePin === pinId1 && conn.targetPin === pinId2) ||
      (conn.sourcePin === pinId2 && conn.targetPin === pinId1)
  );
}

/**
 * Find all connections where a pin is the source
 * 
 * @param connections - Array of connections to search
 * @param pinId - ID of the source pin
 * @returns Array of connections where the pin is the source
 */
export function findConnectionsFromPin(
  connections: Connection[],
  pinId: string
): Connection[] {
  return connections.filter((conn) => conn.sourcePin === pinId);
}

/**
 * Find all connections where a pin is the target
 * 
 * @param connections - Array of connections to search
 * @param pinId - ID of the target pin
 * @returns Array of connections where the pin is the target
 */
export function findConnectionsToPin(
  connections: Connection[],
  pinId: string
): Connection[] {
  return connections.filter((conn) => conn.targetPin === pinId);
}

/**
 * Get all target pins connected to a source pin
 * 
 * @param connections - Array of connections to search
 * @param sourcePinId - ID of the source pin
 * @returns Array of target pin IDs
 */
export function getTargetPins(
  connections: Connection[],
  sourcePinId: string
): string[] {
  return connections
    .filter((conn) => conn.sourcePin === sourcePinId)
    .map((conn) => conn.targetPin);
}

/**
 * Get the source pin connected to a target pin
 * 
 * @param connections - Array of connections to search
 * @param targetPinId - ID of the target pin
 * @returns Source pin ID if found, null otherwise
 * 
 * Note: Input pins should only have one connection, so this returns a single value
 */
export function getSourcePin(
  connections: Connection[],
  targetPinId: string
): string | null {
  const conn = connections.find((c) => c.targetPin === targetPinId);
  return conn ? conn.sourcePin : null;
}

/**
 * Count connections for a specific pin
 * 
 * @param connections - Array of connections to search
 * @param pinId - ID of the pin
 * @returns Number of connections involving the pin
 */
export function countConnectionsForPin(
  connections: Connection[],
  pinId: string
): number {
  return connections.filter(
    (conn) => conn.sourcePin === pinId || conn.targetPin === pinId
  ).length;
}

/**
 * Check if a pin has any connections
 * 
 * @param connections - Array of connections to search
 * @param pinId - ID of the pin
 * @returns True if the pin has at least one connection
 */
export function hasConnections(
  connections: Connection[],
  pinId: string
): boolean {
  return connections.some(
    (conn) => conn.sourcePin === pinId || conn.targetPin === pinId
  );
}

/**
 * Remove connections involving a specific pin
 * 
 * @param connections - Array of connections
 * @param pinId - ID of the pin
 * @returns New array with connections removed
 */
export function removeConnectionsForPin(
  connections: Connection[],
  pinId: string
): Connection[] {
  return connections.filter(
    (conn) => conn.sourcePin !== pinId && conn.targetPin !== pinId
  );
}

/**
 * Remove connections involving a specific node
 * 
 * @param connections - Array of connections
 * @param node - Node to remove connections for
 * @returns New array with connections removed
 */
export function removeConnectionsForNode(
  connections: Connection[],
  node: BaseNode
): Connection[] {
  const pinIds = new Set<string>();
  node.inputs.forEach((pin) => pinIds.add(pin.id));
  node.outputs.forEach((pin) => pinIds.add(pin.id));
  
  return connections.filter(
    (conn) => !pinIds.has(conn.sourcePin) && !pinIds.has(conn.targetPin)
  );
}

/**
 * Validate that all connections reference valid pins
 * 
 * @param connections - Array of connections to validate
 * @param nodes - Array of nodes containing the pins
 * @returns Array of error messages (empty if valid)
 */
export function validateConnections(
  connections: Connection[],
  nodes: BaseNode[]
): string[] {
  const errors: string[] = [];
  
  // Build maps of pin IDs to pins
  const pinMap = new Map<string, { pin: any; node: BaseNode }>();
  nodes.forEach((node) => {
    node.inputs.forEach((pin) => pinMap.set(pin.id, { pin, node }));
    node.outputs.forEach((pin) => pinMap.set(pin.id, { pin, node }));
  });
  
  // Check each connection
  connections.forEach((conn) => {
    const sourceInfo = pinMap.get(conn.sourcePin);
    const targetInfo = pinMap.get(conn.targetPin);
    
    // Check if pins exist
    if (!sourceInfo) {
      errors.push(
        `Connection ${conn.id} references invalid source pin: ${conn.sourcePin}`
      );
      return;
    }
    if (!targetInfo) {
      errors.push(
        `Connection ${conn.id} references invalid target pin: ${conn.targetPin}`
      );
      return;
    }
    
    // Check pin directions
    const sourcePin = sourceInfo.pin;
    const targetPin = targetInfo.pin;
    
    if (sourcePin.direction !== 'output') {
      errors.push(
        `Connection ${conn.id}: source pin ${conn.sourcePin} is not an output pin`
      );
    }
    if (targetPin.direction !== 'input') {
      errors.push(
        `Connection ${conn.id}: target pin ${conn.targetPin} is not an input pin`
      );
    }
    
    // Check type compatibility (if types are defined)
    if (sourcePin.type && targetPin.type) {
      // Allow 'any' type to connect to anything
      if (sourcePin.type !== 'any' && targetPin.type !== 'any') {
        if (sourcePin.type !== targetPin.type) {
          errors.push(
            `Connection ${conn.id}: type mismatch (${sourcePin.type} -> ${targetPin.type})`
          );
        }
      }
    }
  });
  
  return errors;
}

/**
 * Check if connections are valid (convenience function)
 * 
 * @param connections - Array of connections to validate
 * @param nodes - Array of nodes containing the pins
 * @returns True if all connections are valid
 */
export function areConnectionsValid(
  connections: Connection[],
  nodes: BaseNode[]
): boolean {
  return validateConnections(connections, nodes).length === 0;
}
