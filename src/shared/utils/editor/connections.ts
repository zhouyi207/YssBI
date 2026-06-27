/**
 * ConnectionItem query helper functions
 * 
 * These functions provide convenient ways to query connections in a subgraph.
 * They work with the connections array (single source of truth) rather than Pin.links.
 */

import { ConnectionItem, Pin } from '@/shared/types/domain';
import { Node } from '@/shared/types/ui';
import { canConnectPins } from '@/shared/utils/pinCompatibility';

/**
 * Find all connections that involve a specific pin
 * 
 * @param connections - Array of connections to search
 * @param pinId - ID of the pin to find connections for
 * @returns Array of connections where the pin is either source or target
 */
export function findConnectionsByPin(
  connections: ConnectionItem[],
  pinId: string
): ConnectionItem[] {
  return connections.filter(
    (conn) => conn.fromPin === pinId || conn.toPin === pinId
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
  connections: ConnectionItem[],
  node: Node
): ConnectionItem[] {
  // Get all pin IDs for this node
  const pinIds = new Set<string>();
  
  node.inputs.forEach((pin) => pinIds.add(pin.id));
  node.outputs.forEach((pin) => pinIds.add(pin.id));
  
  // Find connections involving any of these pins
  return connections.filter(
    (conn) => pinIds.has(conn.fromPin) || pinIds.has(conn.toPin)
  );
}

/**
 * Find a ConnectionItem by its ID
 * 
 * @param connections - Array of connections to search
 * @param connectionId - ID of the ConnectionItem to find
 * @returns The ConnectionItem if found, null otherwise
 */
export function findConnectionById(
  connections: ConnectionItem[],
  fromPin: string,
  toPin: string
): ConnectionItem | null {
  return connections.find((conn) => conn.fromPin === fromPin && conn.toPin === toPin) || null;
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
  connections: ConnectionItem[],
  pinId1: string,
  pinId2: string
): boolean {
  return connections.some(
    (conn) =>
      (conn.fromPin === pinId1 && conn.toPin === pinId2) ||
      (conn.fromPin === pinId2 && conn.toPin === pinId1)
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
  connections: ConnectionItem[],
  pinId: string
): ConnectionItem[] {
  return connections.filter((conn) => conn.fromPin === pinId);
}

/**
 * Find all connections where a pin is the target
 * 
 * @param connections - Array of connections to search
 * @param pinId - ID of the target pin
 * @returns Array of connections where the pin is the target
 */
export function findConnectionsToPin(
  connections: ConnectionItem[],
  pinId: string
): ConnectionItem[] {
  return connections.filter((conn) => conn.toPin === pinId);
}

/**
 * Get all target pins connected to a source pin
 * 
 * @param connections - Array of connections to search
 * @param sourcePinId - ID of the source pin
 * @returns Array of target pin IDs
 */
export function getTargetPins(
  connections: ConnectionItem[],
  sourcePinId: string
): string[] {
  return connections
    .filter((conn) => conn.fromPin === sourcePinId)
    .map((conn) => conn.toPin);
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
  connections: ConnectionItem[],
  targetPinId: string
): string | null {
  const conn = connections.find((c) => c.toPin === targetPinId);
  return conn ? conn.fromPin : null;
}

/**
 * Count connections for a specific pin
 * 
 * @param connections - Array of connections to search
 * @param pinId - ID of the pin
 * @returns Number of connections involving the pin
 */
export function countConnectionsForPin(
  connections: ConnectionItem[],
  pinId: string
): number {
  return connections.filter(
    (conn) => conn.fromPin === pinId || conn.toPin === pinId
  ).length;
}

/**
 * Check if a pin has any connections
 * 
 * @param connections - Array of connections to search
 * @param pinId - ID of the pin
 * @returns True if the pin has at least one ConnectionItem
 */
export function hasConnections(
  connections: ConnectionItem[],
  pinId: string
): boolean {
  return connections.some(
    (conn) => conn.fromPin === pinId || conn.toPin === pinId
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
  connections: ConnectionItem[],
  pinId: string
): ConnectionItem[] {
  return connections.filter(
    (conn) => conn.fromPin !== pinId && conn.toPin !== pinId
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
  connections: ConnectionItem[],
  node: Node
): ConnectionItem[] {
  const pinIds = new Set<string>();
  node.inputs.forEach((pin) => pinIds.add(pin.id));
  node.outputs.forEach((pin) => pinIds.add(pin.id));
  
  return connections.filter(
    (conn) => !pinIds.has(conn.fromPin) && !pinIds.has(conn.toPin)
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
  connections: ConnectionItem[],
  nodes: Node[]
): string[] {
  const errors: string[] = [];
  
  // Build maps of pin IDs to pins
  const pinMap = new Map<string, { pin: Pin; node: Node }>();
  nodes.forEach((node) => {
    node.inputs.forEach((pin) => pinMap.set(pin.id, { pin, node }));
    node.outputs.forEach((pin) => pinMap.set(pin.id, { pin, node }));
  });
  
  // Check each ConnectionItem
  connections.forEach((conn) => {
    const sourceInfo = pinMap.get(conn.fromPin);
    const targetInfo = pinMap.get(conn.toPin);
    
    // Check if pins exist
    if (!sourceInfo) {
      errors.push(
        `ConnectionItem ${conn.fromPin}->${conn.toPin} references invalid source pin: ${conn.fromPin}`
      );
      return;
    }
    if (!targetInfo) {
      errors.push(
        `ConnectionItem ${conn.fromPin}->${conn.toPin} references invalid target pin: ${conn.toPin}`
      );
      return;
    }
    
    // Check pin directions
    const from_pin = sourceInfo.pin;
    const to_pin = targetInfo.pin;
    
    if (from_pin.direction !== 'output') {
      errors.push(
        `ConnectionItem ${conn.fromPin}->${conn.toPin}: source pin ${conn.fromPin} is not an output pin`
      );
    }
    if (to_pin.direction !== 'input') {
      errors.push(
        `ConnectionItem ${conn.fromPin}->${conn.toPin}: target pin ${conn.toPin} is not an input pin`
      );
    }
    
    if (!canConnectPins(from_pin, to_pin)) {
      errors.push(
        `ConnectionItem ${conn.fromPin}->${conn.toPin}: type mismatch (${from_pin.type} -> ${to_pin.type})`
      );
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
  connections: ConnectionItem[],
  nodes: Node[]
): boolean {
  return validateConnections(connections, nodes).length === 0;
}
