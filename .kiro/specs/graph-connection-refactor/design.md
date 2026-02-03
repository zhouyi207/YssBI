# Design Document: Graph Connection Refactor

## Overview

This design document describes the refactoring of the graph serialization architecture to separate connection relationships from Pin objects. The current architecture stores connections as a `links: string[]` field within Pin objects, which creates data redundancy, consistency risks, and complications during copy/paste operations. The new architecture introduces independent Connection entities that serve as the single source of truth for all connection relationships.

### Current Architecture Problems

1. **Data Redundancy**: Each connection is stored twice (once in each connected pin's `links` array)
2. **Consistency Risks**: Updates must modify both pins, creating opportunities for inconsistent state
3. **Copy/Paste Issues**: Serialized nodes may contain links to pins outside the selection, creating dangling references
4. **Complex Operations**: Creating/deleting connections requires finding and updating multiple pin objects

### New Architecture Benefits

1. **Single Source of Truth**: Each connection stored exactly once
2. **Atomic Operations**: Connection CRUD operations are simple and atomic
3. **Clean Serialization**: Copy/paste automatically filters external connections
4. **Simplified Logic**: Connection management is centralized and straightforward

## Architecture

### High-Level Structure

```
SubGraph
├── nodes: BaseNode[]
└── connections: Connection[]

BaseNode
├── inputs: Pin[]
└── outputs: Pin[]

Pin
├── id: string
├── name: string
├── type: PinType
└── (runtime only) links: string[]  // Populated during deserialization

Connection
├── id: string
├── sourcePin: string  // Pin ID
└── targetPin: string  // Pin ID
```

### Data Flow

**Serialization Flow:**
```
Runtime State (Nodes with Pins)
    ↓
Extract connections from Pin.links
    ↓
SubGraphData { nodes, connections }
    ↓
JSON / Backend Storage
```

**Deserialization Flow:**
```
JSON / Backend Storage
    ↓
SubGraphData { nodes, connections }
    ↓
Rebuild Pin.links from connections (for runtime)
    ↓
Runtime State (Nodes with Pins)
```

### Key Design Decisions

1. **Runtime Links Field**: Pins will have a `links` field at runtime for efficient querying, but this field is NOT serialized. It's rebuilt during deserialization from the connections array.

2. **Connection Filtering**: During copy/paste, connections are automatically filtered to only include those where both source and target pins belong to selected nodes.

3. **Backend Synchronization**: The backend will maintain the same Connection structure, ensuring consistency between frontend and backend representations.

4. **Backward Compatibility**: This refactor does NOT maintain backward compatibility with old save files (as requested by the user).

## Components and Interfaces

### Frontend Types (TypeScript)

#### Connection Interface

```typescript
export interface Connection {
  id: string;           // Unique identifier for the connection
  sourcePin: string;    // ID of the source (output) pin
  targetPin: string;    // ID of the target (input) pin
}
```

#### Updated SubGraphData Interface

```typescript
export interface SubGraphData {
  id: string;
  name: string;
  type: "event" | "function" | "macro";
  canvas: CanvasState;
  variables: Record<string, VariableDefinition>;
  inputs: PinDefinition[];
  outputs: PinDefinition[];
  nodes: SerializedNode[];
  connections: Connection[];  // NEW: Separate connections array
}
```

#### Updated Pin Interface

```typescript
export interface Pin {
  id: string;
  nodeId: string;
  name: string;
  type: PinType;
  direction: PinDirection;
  // links field is REMOVED from serialization
  // but exists at runtime for efficient querying
  links?: string[];  // Runtime only, not serialized
  defaultValue?: any;
  userValue?: any;
  isArray?: boolean;
  ui?: {
    x?: number;
    y?: number;
    color?: string;
  };
}
```

#### Serialized Node Format

```typescript
interface SerializedNode {
  id: string;
  type: string;
  title: string;
  position: Position;
  isInternal?: boolean;
  variableId?: string;
  variableType?: string;
  variableName?: string;
  subGraphId?: string;
  inputs: SerializedPin[];
  outputs: SerializedPin[];
}

interface SerializedPin {
  id: string;
  name: string;
  type: PinType;
  // links field is NOT included in serialization
  defaultValue?: any;
  userValue?: any;
  isArray?: boolean;
}
```

### Backend Types (Rust)

#### Connection DTO

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectionDto {
    pub id: String,
    #[serde(rename = "sourcePin")]
    pub source_pin: String,
    #[serde(rename = "targetPin")]
    pub target_pin: String,
}
```

#### Updated SubGraph Structure

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubGraph {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub subgraph_type: String,
    pub nodes: Vec<SerializedNode>,
    pub connections: Vec<ConnectionDto>,  // NEW
    pub canvas: CanvasState,
    pub variables: HashMap<String, VariableDto>,
    pub inputs: Vec<PinDefDto>,
    pub outputs: Vec<PinDefDto>,
}
```

#### Updated Pin DTO

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PinDto {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    // links field REMOVED from serialization
    #[serde(rename = "defaultValue", skip_serializing_if = "Option::is_none")]
    pub default_value: Option<Value>,
    #[serde(rename = "userValue", skip_serializing_if = "Option::is_none")]
    pub user_value: Option<Value>,
    #[serde(rename = "isArray", default)]
    pub is_array: bool,
    #[serde(rename = "showWidget", default = "default_true")]
    pub show_widget: bool,
    #[serde(rename = "widgetType", skip_serializing_if = "Option::is_none")]
    pub widget_type: Option<String>,
}
```

### Connection Operations

#### Frontend Connection Manager

```typescript
class ConnectionManager {
  private connections: Map<string, Connection>;
  
  // Create a new connection
  createConnection(sourcePin: string, targetPin: string): Connection {
    const id = `conn-${crypto.randomUUID()}`;
    const connection: Connection = { id, sourcePin, targetPin };
    this.connections.set(id, connection);
    return connection;
  }
  
  // Delete a connection
  deleteConnection(connectionId: string): boolean {
    return this.connections.delete(connectionId);
  }
  
  // Find connections by pin ID
  findConnectionsByPin(pinId: string): Connection[] {
    return Array.from(this.connections.values())
      .filter(c => c.sourcePin === pinId || c.targetPin === pinId);
  }
  
  // Find connections by node ID
  findConnectionsByNode(nodeId: string, pins: Pin[]): Connection[] {
    const pinIds = new Set(pins.map(p => p.id));
    return Array.from(this.connections.values())
      .filter(c => pinIds.has(c.sourcePin) || pinIds.has(c.targetPin));
  }
  
  // Check if two pins are connected
  areConnected(pin1: string, pin2: string): boolean {
    return Array.from(this.connections.values())
      .some(c => 
        (c.sourcePin === pin1 && c.targetPin === pin2) ||
        (c.sourcePin === pin2 && c.targetPin === pin1)
      );
  }
  
  // Get all connections
  getAllConnections(): Connection[] {
    return Array.from(this.connections.values());
  }
}
```

#### Backend Connection Operations

```rust
impl ProjectState {
    /// Create a connection between two pins
    pub fn create_connection(
        &self,
        subgraph_id: &str,
        source_pin_id: &str,
        target_pin_id: &str,
    ) -> Result<ConnectionDto, String> {
        // Validate pins exist and have correct directions
        // Validate type compatibility
        // Create connection
        let connection = ConnectionDto {
            id: format!("conn-{}", uuid::Uuid::new_v4()),
            source_pin: source_pin_id.to_string(),
            target_pin: target_pin_id.to_string(),
        };
        
        // Add to subgraph
        let mut project = self.data.write().unwrap();
        let subgraph = get_subgraph_mut!(project, subgraph_id)?;
        subgraph.connections.push(connection.clone());
        
        Ok(connection)
    }
    
    /// Delete a connection
    pub fn delete_connection(
        &self,
        subgraph_id: &str,
        connection_id: &str,
    ) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        let subgraph = get_subgraph_mut!(project, subgraph_id)?;
        
        let original_len = subgraph.connections.len();
        subgraph.connections.retain(|c| c.id != connection_id);
        
        if subgraph.connections.len() == original_len {
            return Err(format!("Connection '{}' not found", connection_id));
        }
        
        Ok(())
    }
    
    /// Delete all connections for a pin
    pub fn delete_connections_for_pin(
        &self,
        subgraph_id: &str,
        pin_id: &str,
    ) -> Result<Vec<String>, String> {
        let mut project = self.data.write().unwrap();
        let subgraph = get_subgraph_mut!(project, subgraph_id)?;
        
        let removed_ids: Vec<String> = subgraph.connections
            .iter()
            .filter(|c| c.source_pin == pin_id || c.target_pin == pin_id)
            .map(|c| c.id.clone())
            .collect();
        
        subgraph.connections.retain(|c| 
            c.source_pin != pin_id && c.target_pin != pin_id
        );
        
        Ok(removed_ids)
    }
    
    /// Delete all connections for a node
    pub fn delete_connections_for_node(
        &self,
        subgraph_id: &str,
        node_id: &str,
    ) -> Result<Vec<String>, String> {
        let mut project = self.data.write().unwrap();
        let subgraph = get_subgraph_mut!(project, subgraph_id)?;
        
        // Find all pin IDs for this node
        let pin_ids: HashSet<String> = subgraph.nodes
            .iter()
            .filter(|n| n.id == node_id)
            .flat_map(|n| {
                n.inputs.iter()
                    .chain(n.outputs.iter())
                    .map(|p| p.id.clone())
            })
            .collect();
        
        let removed_ids: Vec<String> = subgraph.connections
            .iter()
            .filter(|c| pin_ids.contains(&c.source_pin) || pin_ids.contains(&c.target_pin))
            .map(|c| c.id.clone())
            .collect();
        
        subgraph.connections.retain(|c| 
            !pin_ids.contains(&c.source_pin) && !pin_ids.contains(&c.target_pin)
        );
        
        Ok(removed_ids)
    }
}
```

## Data Models

### Connection Storage

Connections are stored in a flat array within SubGraphData:

```typescript
{
  id: "subgraph-123",
  name: "My Event",
  type: "event",
  nodes: [...],
  connections: [
    { id: "conn-1", sourcePin: "pin-out-1", targetPin: "pin-in-2" },
    { id: "conn-2", sourcePin: "pin-out-2", targetPin: "pin-in-3" },
    { id: "conn-3", sourcePin: "pin-out-1", targetPin: "pin-in-4" }
  ],
  canvas: {...},
  variables: {...}
}
```

### Runtime Pin Links

At runtime, Pin objects maintain a `links` array for efficient querying:

```typescript
// After deserialization, pins are populated with links
{
  id: "pin-out-1",
  nodeId: "node-1",
  name: "Output",
  type: "int",
  direction: "output",
  links: ["pin-in-2", "pin-in-4"]  // Populated from connections array
}
```

This runtime field is rebuilt during deserialization and kept synchronized during editing operations.

### Serialization Format

#### Before (Current):

```json
{
  "nodes": [
    {
      "id": "node-1",
      "inputs": [],
      "outputs": [
        {
          "id": "pin-out-1",
          "name": "Result",
          "type": "int",
          "links": ["pin-in-2", "pin-in-4"]
        }
      ]
    },
    {
      "id": "node-2",
      "inputs": [
        {
          "id": "pin-in-2",
          "name": "Value",
          "type": "int",
          "links": ["pin-out-1"]
        }
      ],
      "outputs": []
    }
  ]
}
```

#### After (New):

```json
{
  "nodes": [
    {
      "id": "node-1",
      "inputs": [],
      "outputs": [
        {
          "id": "pin-out-1",
          "name": "Result",
          "type": "int"
        }
      ]
    },
    {
      "id": "node-2",
      "inputs": [
        {
          "id": "pin-in-2",
          "name": "Value",
          "type": "int"
        }
      ],
      "outputs": []
    }
  ],
  "connections": [
    {
      "id": "conn-1",
      "sourcePin": "pin-out-1",
      "targetPin": "pin-in-2"
    },
    {
      "id": "conn-2",
      "sourcePin": "pin-out-1",
      "targetPin": "pin-in-4"
    }
  ]
}
```

## Correctness Properties


*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Serialization Properties

**Property 1: Serialized pins exclude links field**

*For any* subgraph with connections, when serialized, all pin objects in the serialized output should not contain a `links` field.

**Validates: Requirements 1.3, 12.2**

**Property 2: Serialized subgraphs include connections array**

*For any* subgraph, when serialized, the resulting SubGraphData should contain a `connections` array.

**Validates: Requirements 1.4**

**Property 3: Serialization extracts all connections**

*For any* subgraph with runtime pin links, when serialized, all connections represented in pin links should appear exactly once in the connections array and nowhere else.

**Validates: Requirements 2.1, 2.3**

**Property 4: Serialization round-trip preserves structure**

*For any* valid subgraph, serializing then deserializing should produce a graph with equivalent connection topology (same nodes connected to same nodes).

**Validates: Requirements 2.2**

**Property 5: Frontend-backend serialization compatibility**

*For any* subgraph, serializing on the frontend and deserializing on the backend (or vice versa) should produce equivalent connection structures.

**Validates: Requirements 2.4, 10.4**

### Copy/Paste Properties

**Property 6: Copy includes only internal connections**

*For any* graph and any subset of nodes, copying those nodes should include exactly the connections where both source and target pins belong to the selected nodes, and exclude all connections referencing external pins.

**Validates: Requirements 3.1, 3.2**

**Property 7: Paste preserves internal topology**

*For any* set of copied nodes with internal connections, pasting should create new nodes with the same internal connection topology as the original selection.

**Validates: Requirements 3.3, 3.4**

### Connection Creation Properties

**Property 8: Connection creation is atomic**

*For any* valid source and target pin pair, creating a connection should add exactly one Connection object to the connections collection and should not modify any Pin objects in the serialized format.

**Validates: Requirements 4.1, 4.2**

**Property 9: Connection creation validates pin existence**

*For any* pin ID pair, attempting to create a connection should succeed if and only if both pins exist in the graph.

**Validates: Requirements 4.3**

**Property 10: Duplicate connections are prevented**

*For any* existing connection between two pins, attempting to create another connection between the same pins should be rejected.

**Validates: Requirements 4.4**

### Connection Deletion Properties

**Property 11: Connection deletion is atomic**

*For any* existing connection, deleting it should remove exactly one Connection object from the connections collection and should not modify any Pin objects in the serialized format.

**Validates: Requirements 5.1, 5.2**

**Property 12: Node deletion removes associated connections**

*For any* node in a graph, deleting that node should remove all connections that reference any pin on that node, ensuring no orphaned connections remain.

**Validates: Requirements 5.3, 5.4, 11.3**

### Connection Query Properties

**Property 13: Find connections by pin**

*For any* graph and any pin ID, querying connections for that pin should return exactly the set of connections where the pin appears as either source or target.

**Validates: Requirements 6.1**

**Property 14: Find connections by node**

*For any* graph and any node ID, querying connections for that node should return exactly the set of connections that reference any pin on that node.

**Validates: Requirements 6.2**

**Property 15: Find connection by ID**

*For any* connection in a graph, looking it up by its ID should return that exact connection.

**Validates: Requirements 6.3**

**Property 16: Connection existence check**

*For any* two pins in a graph, checking if they are connected should return true if and only if a connection exists between them (in either direction).

**Validates: Requirements 6.4**

### Executor Properties

**Property 17: Executor reads from connections collection**

*For any* graph execution, the executor should determine data flow by reading the connections collection, and should function correctly even when Pin.links is empty or missing.

**Validates: Requirements 8.1, 8.3**

**Property 18: Executor propagates data correctly**

*For any* graph with connections and input data, executing the graph should propagate data from source pins to target pins according to the connections array.

**Validates: Requirements 8.2, 8.4**

### Undo/Redo Properties

**Property 19: Connection creation undo/redo**

*For any* connection creation operation, undoing should remove the connection, and redoing should restore it to the exact same state.

**Validates: Requirements 9.1, 9.3, 9.4**

**Property 20: Connection deletion undo/redo**

*For any* connection deletion operation, undoing should restore the connection, and redoing should remove it again.

**Validates: Requirements 9.2**

### Backend API Properties

**Property 21: Backend connection creation**

*For any* valid pin pair, calling the backend connect_pins function should create exactly one Connection object with valid structure.

**Validates: Requirements 10.1**

**Property 22: Backend connection deletion**

*For any* existing connection, calling the backend disconnect_pin function should remove all connections referencing that pin.

**Validates: Requirements 10.2**

**Property 23: Backend connection validation**

*For any* connection creation attempt, the backend should validate pin existence, direction compatibility, and type compatibility, rejecting invalid connections.

**Validates: Requirements 10.3**

### Data Consistency Properties

**Property 24: Connections reference valid pins**

*For any* graph state (after any operation), all connections in the connections array should reference pin IDs that exist in the graph's nodes.

**Validates: Requirements 11.1, 11.2**

**Property 25: Pin direction validation**

*For any* connection creation attempt, the system should reject connections between pins of incompatible directions (input-to-input or output-to-output).

**Validates: Requirements 11.4**

### Runtime Links Properties

**Property 26: Deserialization populates runtime links**

*For any* serialized subgraph with connections, deserializing should populate Pin.links fields such that each pin's links array matches the connections that reference it.

**Validates: Requirements 12.1**

**Property 27: Runtime links stay synchronized**

*For any* graph state during editing, the runtime Pin.links fields should always match the connections array (each pin's links should contain exactly the pins it's connected to according to the connections array).

**Validates: Requirements 12.3**

**Property 28: Runtime links are read-only**

*For any* modification to Pin.links at runtime, the connections array should remain unchanged, confirming that connections array is the source of truth.

**Validates: Requirements 12.4**

## Error Handling

### Connection Validation Errors

1. **Invalid Pin Reference**: When creating a connection with non-existent pin IDs
   - Error: `"Pin '{pin_id}' not found in graph"`
   - Action: Reject connection creation

2. **Direction Mismatch**: When attempting to connect incompatible pin directions
   - Error: `"Cannot connect: pins must have different directions (one input, one output)"`
   - Action: Reject connection creation

3. **Type Incompatibility**: When attempting to connect incompatible pin types
   - Error: `"Cannot connect: type '{source_type}' is not compatible with type '{target_type}'"`
   - Action: Reject connection creation

4. **Duplicate Connection**: When attempting to create a connection that already exists
   - Error: `"Connection already exists between '{source_pin}' and '{target_pin}'"`
   - Action: Reject connection creation

### Node Deletion Cascading

When a node is deleted:
1. Identify all pins on the node
2. Find all connections referencing those pins
3. Remove all identified connections
4. Remove the node
5. Update runtime Pin.links on remaining nodes

### Serialization Errors

1. **Missing Connections Array**: When deserializing data without connections array
   - Action: Initialize empty connections array, log warning
   - Fallback: Attempt to extract connections from legacy Pin.links format

2. **Invalid Connection Reference**: When a connection references non-existent pins
   - Action: Skip invalid connection, log warning
   - Continue deserializing valid connections

### Backend Synchronization Errors

1. **Connection Creation Failure**: When backend rejects connection creation
   - Action: Revert frontend state, show error toast
   - Ensure frontend and backend stay synchronized

2. **Connection Deletion Failure**: When backend fails to delete connection
   - Action: Revert frontend state, show error toast
   - Retry or manual intervention required

## Testing Strategy

### Dual Testing Approach

This refactor requires both unit tests and property-based tests to ensure comprehensive coverage:

**Unit Tests** focus on:
- Specific examples of connection operations
- Edge cases (empty graphs, single node, disconnected nodes)
- Error conditions (invalid pins, type mismatches)
- Integration points (frontend-backend communication)

**Property-Based Tests** focus on:
- Universal properties across all possible graphs
- Serialization/deserialization round-trips
- Copy/paste with various node selections
- Connection CRUD operations with random graphs

### Property-Based Testing Configuration

- **Library**: Use `fast-check` for TypeScript/JavaScript, `proptest` for Rust
- **Iterations**: Minimum 100 iterations per property test
- **Tagging**: Each property test must reference its design document property
- **Tag Format**: `// Feature: graph-connection-refactor, Property {number}: {property_text}`

### Test Coverage Areas

#### 1. Serialization Tests

**Unit Tests:**
- Serialize empty graph → verify connections array exists
- Serialize graph with one connection → verify format
- Serialize graph with multiple connections → verify all present
- Deserialize and verify Pin.links populated correctly

**Property Tests:**
- Property 1: Serialized pins exclude links field
- Property 2: Serialized subgraphs include connections array
- Property 3: Serialization extracts all connections
- Property 4: Serialization round-trip preserves structure
- Property 5: Frontend-backend serialization compatibility

#### 2. Copy/Paste Tests

**Unit Tests:**
- Copy single node with no connections
- Copy two connected nodes
- Copy nodes with external connections → verify filtering
- Paste and verify new IDs generated

**Property Tests:**
- Property 6: Copy includes only internal connections
- Property 7: Paste preserves internal topology

#### 3. Connection CRUD Tests

**Unit Tests:**
- Create connection between valid pins
- Attempt to create connection with invalid pin → verify error
- Delete connection → verify removed
- Delete node → verify connections removed

**Property Tests:**
- Property 8: Connection creation is atomic
- Property 9: Connection creation validates pin existence
- Property 10: Duplicate connections are prevented
- Property 11: Connection deletion is atomic
- Property 12: Node deletion removes associated connections

#### 4. Connection Query Tests

**Unit Tests:**
- Find connections for pin with no connections
- Find connections for pin with multiple connections
- Find connections for node
- Check if pins are connected

**Property Tests:**
- Property 13: Find connections by pin
- Property 14: Find connections by node
- Property 15: Find connection by ID
- Property 16: Connection existence check

#### 5. Executor Integration Tests

**Unit Tests:**
- Execute simple graph with one connection
- Execute graph with multiple connections
- Execute graph with branching connections
- Verify data flows correctly

**Property Tests:**
- Property 17: Executor reads from connections collection
- Property 18: Executor propagates data correctly

#### 6. Undo/Redo Tests

**Unit Tests:**
- Create connection → undo → verify removed
- Delete connection → undo → verify restored
- Create → undo → redo → verify restored

**Property Tests:**
- Property 19: Connection creation undo/redo
- Property 20: Connection deletion undo/redo

#### 7. Backend API Tests

**Unit Tests:**
- Call connect_pins with valid pins
- Call connect_pins with invalid pins → verify error
- Call disconnect_pin → verify removed
- Verify backend validation logic

**Property Tests:**
- Property 21: Backend connection creation
- Property 22: Backend connection deletion
- Property 23: Backend connection validation

#### 8. Data Consistency Tests

**Unit Tests:**
- Verify all connections reference valid pins after operations
- Attempt invalid direction connection → verify rejected
- Attempt invalid type connection → verify rejected

**Property Tests:**
- Property 24: Connections reference valid pins
- Property 25: Pin direction validation

#### 9. Runtime Links Tests

**Unit Tests:**
- Deserialize → verify Pin.links populated
- Create connection → verify Pin.links updated
- Delete connection → verify Pin.links updated

**Property Tests:**
- Property 26: Deserialization populates runtime links
- Property 27: Runtime links stay synchronized
- Property 28: Runtime links are read-only

### Test Data Generation

For property-based tests, generate:
- Random graphs with varying numbers of nodes (0-50)
- Random connections between compatible pins
- Random node selections for copy/paste tests
- Random pin types ensuring some compatible pairs exist
- Random operation sequences (create, delete, undo, redo)

### Integration Testing

1. **Frontend-Backend Round-Trip**:
   - Create graph in frontend
   - Serialize and send to backend
   - Backend deserializes and re-serializes
   - Frontend deserializes backend response
   - Verify equivalence

2. **Copy/Paste Across Subgraphs**:
   - Copy nodes from one subgraph
   - Paste into different subgraph
   - Verify connections filtered correctly
   - Verify new IDs generated

3. **Executor Integration**:
   - Create graph with connections
   - Execute graph
   - Verify data flows through connections
   - Verify executor doesn't use Pin.links

### Performance Testing

While not property-based, performance tests should verify:
- Serialization/deserialization performance with large graphs (1000+ nodes)
- Connection query performance with many connections (10000+ connections)
- Copy/paste performance with large selections (100+ nodes)

## Implementation Notes

### Migration Strategy

Since backward compatibility is not required:

1. **Update Type Definitions**: Add Connection interface, update SubGraphData
2. **Update Serialization**: Modify serializeSubGraph to extract connections
3. **Update Deserialization**: Modify deserializeSubGraph to rebuild Pin.links
4. **Update Backend DTOs**: Add ConnectionDto, update SubGraph structure
5. **Update Backend Operations**: Refactor connect_pins and disconnect_pin
6. **Update Frontend Operations**: Modify connection creation/deletion logic
7. **Update Copy/Paste**: Add connection filtering logic
8. **Update Executor**: Modify to read from connections array
9. **Update Undo/Redo**: Track connection operations
10. **Update Tests**: Add property-based tests for all properties

### Phased Rollout

**Phase 1: Data Structures**
- Define Connection types (frontend and backend)
- Update SubGraphData and SubGraph structures
- Update serialization format (no links in pins)

**Phase 2: Serialization/Deserialization**
- Implement connection extraction during serialization
- Implement Pin.links population during deserialization
- Add round-trip tests

**Phase 3: Backend Operations**
- Refactor connect_pins to create Connection objects
- Refactor disconnect_pin to remove Connection objects
- Add connection CRUD operations
- Update node deletion to cascade to connections

**Phase 4: Frontend Integration**
- Update Canvas rendering to use connections
- Update connection creation/deletion UI logic
- Update copy/paste to filter connections
- Update undo/redo to track connections

**Phase 5: Executor Integration**
- Update executor to read from connections array
- Remove executor dependency on Pin.links
- Add executor integration tests

**Phase 6: Testing and Validation**
- Add all property-based tests
- Run comprehensive test suite
- Performance testing
- Manual testing of all workflows

### Key Implementation Considerations

1. **Runtime Links Synchronization**: Ensure Pin.links is updated whenever connections change during editing
2. **ID Generation**: Use consistent ID generation (crypto.randomUUID() for frontend, uuid::Uuid for backend)
3. **Type Safety**: Use TypeScript and Rust type systems to enforce Connection structure
4. **Error Handling**: Provide clear error messages for all validation failures
5. **Performance**: Use efficient data structures (Map/HashMap) for connection lookups
6. **Testing**: Write property-based tests alongside implementation to catch issues early
