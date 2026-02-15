/**
 * ConnectionItem query helper functions
 * 
 * These functions provide convenient ways to query connections in a subgraph.
 * They work with the connections array (single source of truth) rather than Pin.links.
 */

import { ConnectionItem } from '@/shared/types/domain';
import { Node } from '@/shared/types/ui';

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
    (conn) => conn.from_pin === pinId || conn.to_pin === pinId
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
    (conn) => pinIds.has(conn.from_pin) || pinIds.has(conn.to_pin)
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
  from_pin: string,
  to_pin: string
): ConnectionItem | null {
  return connections.find((conn) => conn.from_pin === from_pin && conn.to_pin === to_pin) || null;
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
      (conn.from_pin === pinId1 && conn.to_pin === pinId2) ||
      (conn.from_pin === pinId2 && conn.to_pin === pinId1)
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
  return connections.filter((conn) => conn.from_pin === pinId);
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
  return connections.filter((conn) => conn.to_pin === pinId);
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
    .filter((conn) => conn.from_pin === sourcePinId)
    .map((conn) => conn.to_pin);
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
  const conn = connections.find((c) => c.to_pin === targetPinId);
  return conn ? conn.from_pin : null;
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
    (conn) => conn.from_pin === pinId || conn.to_pin === pinId
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
    (conn) => conn.from_pin === pinId || conn.to_pin === pinId
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
    (conn) => conn.from_pin !== pinId && conn.to_pin !== pinId
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
    (conn) => !pinIds.has(conn.from_pin) && !pinIds.has(conn.to_pin)
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
  const pinMap = new Map<string, { pin: any; node: Node }>();
  nodes.forEach((node) => {
    node.inputs.forEach((pin) => pinMap.set(pin.id, { pin, node }));
    node.outputs.forEach((pin) => pinMap.set(pin.id, { pin, node }));
  });
  
  // Check each ConnectionItem
  connections.forEach((conn) => {
    const sourceInfo = pinMap.get(conn.from_pin);
    const targetInfo = pinMap.get(conn.to_pin);
    
    // Check if pins exist
    if (!sourceInfo) {
      errors.push(
        `ConnectionItem ${conn.from_pin}->${conn.to_pin} references invalid source pin: ${conn.from_pin}`
      );
      return;
    }
    if (!targetInfo) {
      errors.push(
        `ConnectionItem ${conn.from_pin}->${conn.to_pin} references invalid target pin: ${conn.to_pin}`
      );
      return;
    }
    
    // Check pin directions
    const from_pin = sourceInfo.pin;
    const to_pin = targetInfo.pin;
    
    if (from_pin.direction !== 'output') {
      errors.push(
        `ConnectionItem ${conn.from_pin}->${conn.to_pin}: source pin ${conn.from_pin} is not an output pin`
      );
    }
    if (to_pin.direction !== 'input') {
      errors.push(
        `ConnectionItem ${conn.from_pin}->${conn.to_pin}: target pin ${conn.to_pin} is not an input pin`
      );
    }
    
    // Check type compatibility (if types are defined)
    if (from_pin.type && to_pin.type) {
      // Allow 'any' type to connect to anything
      if (from_pin.type !== 'any' && to_pin.type !== 'any') {
        if (from_pin.type !== to_pin.type) {
          errors.push(
            `ConnectionItem ${conn.from_pin}->${conn.to_pin}: type mismatch (${from_pin.type} -> ${to_pin.type})`
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
  connections: ConnectionItem[],
  nodes: Node[]
): boolean {
  return validateConnections(connections, nodes).length === 0;
}
