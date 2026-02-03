# Requirements Document

## Introduction

This document specifies the requirements for refactoring the graph serialization architecture to separate connection relationships from Pin objects. Currently, the system stores connection relationships as a `links: string[]` field within Pin objects, which causes issues during copy/paste operations where links may reference pins outside the selected nodes. The refactoring will introduce independent Connection entities to resolve these issues and simplify connection management.

## Glossary

- **Pin**: An input or output port on a node that can be connected to other pins
- **Connection**: A directed relationship between two pins (source pin to target pin)
- **Node**: A graph element that contains pins and performs operations
- **SubGraph**: A collection of nodes and connections that can be serialized/deserialized
- **Graph_Editor**: The frontend component that renders and manages the visual graph
- **Executor**: The backend component that executes the graph by following connections
- **Serialization**: The process of converting graph data to a storable format
- **Deserialization**: The process of reconstructing graph data from stored format

## Requirements

### Requirement 1: Independent Connection Data Structure

**User Story:** As a developer, I want connections to be stored independently from Pins, so that connection management is simpler and more maintainable.

#### Acceptance Criteria

1. THE System SHALL define a Connection data structure with id, sourcePin, and targetPin fields
2. THE System SHALL store connections in a separate collection from nodes and pins
3. THE Pin data structure SHALL NOT contain a links field for storing connection references
4. THE SubGraphData structure SHALL include a connections array alongside the nodes array

### Requirement 2: Connection Serialization

**User Story:** As a developer, I want graph serialization to use the Connection structure, so that saved graphs have a clear and consistent format.

#### Acceptance Criteria

1. WHEN serializing a subgraph, THE System SHALL extract all connections into a separate connections array
2. WHEN deserializing a subgraph, THE System SHALL reconstruct the graph from the connections array
3. THE System SHALL ensure serialized data contains no redundant connection information
4. THE System SHALL maintain compatibility between frontend and backend serialization formats

### Requirement 3: Copy/Paste Connection Filtering

**User Story:** As a user, I want copy/paste to only include internal connections, so that pasted nodes don't have dangling references to external nodes.

#### Acceptance Criteria

1. WHEN copying nodes, THE System SHALL identify all connections between the selected nodes
2. WHEN copying nodes, THE System SHALL exclude connections that reference pins outside the selected nodes
3. WHEN pasting nodes, THE System SHALL only create connections between the pasted nodes
4. THE System SHALL ensure pasted node topology matches the original internal connection structure

### Requirement 4: Connection Creation Operations

**User Story:** As a developer, I want to create connections by adding a single Connection object, so that connection operations are atomic and simple.

#### Acceptance Criteria

1. WHEN creating a connection, THE System SHALL add exactly one Connection object to the connections collection
2. WHEN creating a connection, THE System SHALL NOT modify Pin objects to store connection references
3. THE System SHALL validate that both source and target pins exist before creating a connection
4. THE System SHALL prevent duplicate connections between the same source and target pins

### Requirement 5: Connection Deletion Operations

**User Story:** As a developer, I want to delete connections by removing a single Connection object, so that deletion is simple and cannot leave inconsistent state.

#### Acceptance Criteria

1. WHEN deleting a connection, THE System SHALL remove exactly one Connection object from the connections collection
2. WHEN deleting a connection, THE System SHALL NOT need to modify Pin objects
3. WHEN deleting a node, THE System SHALL remove all connections that reference any pin on that node
4. THE System SHALL ensure no orphaned connections remain after node deletion

### Requirement 6: Connection Query Operations

**User Story:** As a developer, I want to query connections efficiently, so that I can find connections by pin, node, or connection ID.

#### Acceptance Criteria

1. THE System SHALL provide a method to find all connections for a given pin ID
2. THE System SHALL provide a method to find all connections for a given node ID
3. THE System SHALL provide a method to find a connection by its ID
4. THE System SHALL provide a method to check if two pins are connected

### Requirement 7: Graph Editor Integration

**User Story:** As a user, I want the graph editor to work correctly with the new Connection architecture, so that visual editing operations function as expected.

#### Acceptance Criteria

1. WHEN rendering the graph, THE Graph_Editor SHALL read connections from the connections collection
2. WHEN dragging a connection between pins, THE Graph_Editor SHALL create a Connection object
3. WHEN deleting a visual connection, THE Graph_Editor SHALL remove the corresponding Connection object
4. THE Graph_Editor SHALL display connections correctly based on the Connection data structure

### Requirement 8: Executor Integration

**User Story:** As a developer, I want the executor to use connections for data flow, so that graph execution follows the new architecture.

#### Acceptance Criteria

1. WHEN executing a graph, THE Executor SHALL read connections from the connections collection
2. WHEN determining data flow, THE Executor SHALL use Connection objects to identify source and target pins
3. THE Executor SHALL NOT read connection information from Pin objects
4. THE Executor SHALL correctly propagate data through connections during execution

### Requirement 9: Undo/Redo Support

**User Story:** As a user, I want undo/redo to work with connections, so that I can revert connection changes.

#### Acceptance Criteria

1. WHEN creating a connection, THE System SHALL record the operation for undo/redo
2. WHEN deleting a connection, THE System SHALL record the operation for undo/redo
3. WHEN undoing a connection creation, THE System SHALL remove the Connection object
4. WHEN redoing a connection creation, THE System SHALL restore the Connection object

### Requirement 10: Backend Connection Operations

**User Story:** As a developer, I want backend connection operations to use Connection entities, so that the backend architecture matches the frontend.

#### Acceptance Criteria

1. THE Backend SHALL implement a connect_pins function that creates Connection objects
2. THE Backend SHALL implement a disconnect_pin function that removes Connection objects
3. THE Backend SHALL implement connection validation logic
4. THE Backend SHALL ensure Connection DTOs match the frontend Connection structure

### Requirement 11: Data Consistency

**User Story:** As a developer, I want the system to maintain connection data consistency, so that the graph state is always valid.

#### Acceptance Criteria

1. THE System SHALL ensure all connections reference valid pin IDs
2. THE System SHALL ensure all connections reference pins that belong to existing nodes
3. WHEN a pin is deleted, THE System SHALL remove all connections referencing that pin
4. THE System SHALL prevent connections between incompatible pin types (input to input, output to output)

### Requirement 12: Runtime Pin Links

**User Story:** As a developer, I want runtime access to pin connections, so that code can efficiently query which pins are connected without searching the entire connections array.

#### Acceptance Criteria

1. WHEN deserializing a graph, THE System SHALL populate a runtime links field on Pin objects
2. THE System SHALL NOT serialize the runtime links field
3. THE System SHALL keep runtime links synchronized with the connections collection during editing
4. THE System SHALL use runtime links only for read operations, not as the source of truth
