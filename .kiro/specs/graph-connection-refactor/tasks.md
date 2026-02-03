# Implementation Plan: Graph Connection Refactor

## Overview

This implementation plan breaks down the refactoring of the graph serialization architecture into discrete, incremental tasks. The refactor separates connection relationships from Pin objects by introducing independent Connection entities. Each task builds on previous work, with checkpoints to ensure stability before proceeding.

## Tasks

- [x] 1. Define Connection data structures (frontend and backend)
  - Create Connection interface in TypeScript with id, sourcePin, targetPin fields
  - Create ConnectionDto struct in Rust with matching fields
  - Update SubGraphData interface to include connections array
  - Update SubGraph struct to include connections vector
  - _Requirements: 1.1, 1.2, 1.4_

- [-] 2. Update serialization format to exclude Pin.links
  - [x] 2.1 Modify PinDto to remove links field from serialization
    - Add `#[serde(skip_serializing)]` to links field in Rust
    - Remove links from SerializedPin interface in TypeScript
    - _Requirements: 1.3_
  
  - [ ] 2.2 Write property test for serialized pins excluding links
    - **Property 1: Serialized pins exclude links field**
    - **Validates: Requirements 1.3, 12.2**
  
  - [ ] 2.3 Write property test for connections array presence
    - **Property 2: Serialized subgraphs include connections array**
    - **Validates: Requirements 1.4**

- [-] 3. Implement connection extraction during serialization
  - [x] 3.1 Update serializeSubGraph to extract connections from Pin.links
    - Iterate through all nodes and pins
    - Build connections array from pin links
    - Ensure each connection appears exactly once
    - Remove links field from serialized pins
    - _Requirements: 2.1, 2.3_
  
  - [x] 3.2 Update backend serialization to match frontend format
    - Ensure SubGraph serialization includes connections array
    - Ensure PinDto serialization excludes links
    - _Requirements: 2.4_
  
  - [ ] 3.3 Write property test for connection extraction
    - **Property 3: Serialization extracts all connections**
    - **Validates: Requirements 2.1, 2.3**
  
  - [ ] 3.4 Write property test for serialization round-trip
    - **Property 4: Serialization round-trip preserves structure**
    - **Validates: Requirements 2.2**
  
  - [ ] 3.5 Write property test for frontend-backend compatibility
    - **Property 5: Frontend-backend serialization compatibility**
    - **Validates: Requirements 2.4, 10.4**

- [x] 4. Checkpoint - Verify serialization works correctly
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 5. Implement connection reconstruction during deserialization
  - [x] 5.1 Update deserializeSubGraph to rebuild Pin.links from connections
    - Read connections array from SubGraphData
    - Populate runtime Pin.links fields for each pin
    - Ensure Pin.links matches connections array
    - _Requirements: 12.1, 12.3_
  
  - [x] 5.2 Update backend deserialization to match frontend
    - Ensure backend can deserialize frontend-serialized data
    - Ensure frontend can deserialize backend-serialized data
    - _Requirements: 2.4_
  
  - [ ] 5.3 Write property test for runtime links population
    - **Property 26: Deserialization populates runtime links**
    - **Validates: Requirements 12.1**
  
  - [ ] 5.4 Write property test for runtime links synchronization
    - **Property 27: Runtime links stay synchronized**
    - **Validates: Requirements 12.3**

- [-] 6. Implement backend connection CRUD operations
  - [x] 6.1 Implement create_connection function
    - Validate source and target pins exist
    - Validate pin directions are compatible (one input, one output)
    - Validate pin types are compatible
    - Create Connection object with generated ID
    - Add to subgraph connections array
    - _Requirements: 4.1, 4.3, 10.1, 10.3, 11.4_
  
  - [x] 6.2 Implement delete_connection function
    - Find connection by ID
    - Remove from subgraph connections array
    - _Requirements: 5.1, 10.2_
  
  - [x] 6.3 Implement delete_connections_for_pin function
    - Find all connections referencing the pin
    - Remove all found connections
    - Return list of removed connection IDs
    - _Requirements: 5.3, 10.2_
  
  - [x] 6.4 Implement delete_connections_for_node function
    - Find all pins on the node
    - Find all connections referencing those pins
    - Remove all found connections
    - Return list of removed connection IDs
    - _Requirements: 5.3, 5.4, 11.3_
  
  - [ ] 6.5 Write property test for connection creation
    - **Property 8: Connection creation is atomic**
    - **Validates: Requirements 4.1, 4.2**
  
  - [ ] 6.6 Write property test for pin existence validation
    - **Property 9: Connection creation validates pin existence**
    - **Validates: Requirements 4.3**
  
  - [ ] 6.7 Write property test for duplicate prevention
    - **Property 10: Duplicate connections are prevented**
    - **Validates: Requirements 4.4**
  
  - [ ] 6.8 Write property test for connection deletion
    - **Property 11: Connection deletion is atomic**
    - **Validates: Requirements 5.1, 5.2**
  
  - [ ] 6.9 Write property test for node deletion cascade
    - **Property 12: Node deletion removes associated connections**
    - **Validates: Requirements 5.3, 5.4, 11.3**

- [-] 7. Refactor backend connect_pins and disconnect_pin functions
  - [x] 7.1 Update connect_pins to use create_connection
    - Replace direct Pin.links manipulation with create_connection call
    - Keep existing validation logic
    - Return updated connections array
    - _Requirements: 10.1_
  
  - [x] 7.2 Update disconnect_pin to use delete_connections_for_pin
    - Replace Pin.links manipulation with delete_connections_for_pin call
    - Return updated connections array
    - _Requirements: 10.2_
  
  - [ ] 7.3 Write property test for backend connection creation
    - **Property 21: Backend connection creation**
    - **Validates: Requirements 10.1**
  
  - [ ] 7.4 Write property test for backend connection deletion
    - **Property 22: Backend connection deletion**
    - **Validates: Requirements 10.2**
  
  - [ ] 7.5 Write property test for backend validation
    - **Property 23: Backend connection validation**
    - **Validates: Requirements 10.3**

- [x] 8. Checkpoint - Verify backend operations work correctly
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 9. Implement connection query operations
  - [x] 9.1 Implement findConnectionsByPin function (frontend)
    - Filter connections where sourcePin or targetPin matches
    - Return array of matching connections
    - _Requirements: 6.1_
  
  - [x] 9.2 Implement findConnectionsByNode function (frontend)
    - Get all pin IDs for the node
    - Filter connections referencing any of those pins
    - Return array of matching connections
    - _Requirements: 6.2_
  
  - [x] 9.3 Implement findConnectionById function (frontend)
    - Look up connection by ID
    - Return connection or null
    - _Requirements: 6.3_
  
  - [x] 9.4 Implement areConnected function (frontend)
    - Check if any connection exists between two pins
    - Return boolean
    - _Requirements: 6.4_
  
  - [ ] 9.5 Write property test for find by pin
    - **Property 13: Find connections by pin**
    - **Validates: Requirements 6.1**
  
  - [ ] 9.6 Write property test for find by node
    - **Property 14: Find connections by node**
    - **Validates: Requirements 6.2**
  
  - [ ] 9.7 Write property test for find by ID
    - **Property 15: Find connection by ID**
    - **Validates: Requirements 6.3**
  
  - [ ] 9.8 Write property test for connection existence check
    - **Property 16: Connection existence check**
    - **Validates: Requirements 6.4**

- [-] 10. Update frontend connection creation and deletion
  - [x] 10.1 Update connectPins function in useCanvasInteraction
    - Call backend create_connection API
    - Update local connections array
    - Update runtime Pin.links on affected pins
    - Save to undo history
    - _Requirements: 4.1, 4.2, 9.1_
  
  - [x] 10.2 Update connection deletion logic
    - Call backend delete_connection API
    - Update local connections array
    - Update runtime Pin.links on affected pins
    - Save to undo history
    - _Requirements: 5.1, 5.2, 9.2_
  
  - [x] 10.3 Update node deletion to cascade to connections
    - Call findConnectionsByNode before deleting node
    - Delete all found connections
    - Delete the node
    - Update runtime Pin.links on remaining nodes
    - _Requirements: 5.3, 5.4_

- [-] 11. Update copy/paste logic to filter connections
  - [x] 11.1 Update copy function to extract internal connections
    - Get selected node IDs
    - Get all pin IDs for selected nodes
    - Filter connections where both sourcePin and targetPin are in selected pins
    - Store filtered connections in clipboard
    - _Requirements: 3.1, 3.2_
  
  - [x] 11.2 Update paste function to restore internal connections
    - Generate new IDs for nodes and pins
    - Create ID mapping (old ID → new ID)
    - Remap connection pin IDs using mapping
    - Create connections via backend API
    - _Requirements: 3.3, 3.4_
  
  - [ ] 11.3 Write property test for copy filtering
    - **Property 6: Copy includes only internal connections**
    - **Validates: Requirements 3.1, 3.2**
  
  - [ ] 11.4 Write property test for paste topology preservation
    - **Property 7: Paste preserves internal topology**
    - **Validates: Requirements 3.3, 3.4**

- [x] 12. Checkpoint - Verify copy/paste works correctly
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 13. Update graph rendering to use connections array
  - [x] 13.1 Update Canvas component to read from connections
    - Iterate through connections array instead of Pin.links
    - Render connection lines based on Connection objects
    - Update connection hover/selection logic
    - _Requirements: 7.1, 7.4_
  
  - [x] 13.2 Update connection drag logic
    - When drag completes, call connectPins with Connection creation
    - Ensure Connection object is created
    - _Requirements: 7.2_
  
  - [x] 13.3 Update connection deletion UI
    - When user deletes connection, remove Connection object
    - Update visual rendering
    - _Requirements: 7.3_

- [ ] 14. Update undo/redo to track connection operations
  - [x] 14.1 Add connection creation to undo history
    - Record connection ID and details
    - On undo: delete the connection
    - On redo: recreate the connection
    - _Requirements: 9.1, 9.3, 9.4_
  
  - [x] 14.2 Add connection deletion to undo history
    - Record connection details before deletion
    - On undo: recreate the connection
    - On redo: delete the connection again
    - _Requirements: 9.2_
  
  - [ ] 14.3 Write property test for connection creation undo/redo
    - **Property 19: Connection creation undo/redo**
    - **Validates: Requirements 9.1, 9.3, 9.4**
  
  - [ ] 14.4 Write property test for connection deletion undo/redo
    - **Property 20: Connection deletion undo/redo**
    - **Validates: Requirements 9.2**

- [-] 15. Update executor to use connections array
  - [x] 15.1 Modify executor to read from connections collection
    - Update data flow analysis to use connections array
    - Remove any dependency on Pin.links
    - Ensure executor works with empty/missing Pin.links
    - _Requirements: 8.1, 8.3_
  
  - [x] 15.2 Update data propagation logic
    - Use Connection objects to determine source and target pins
    - Propagate data through connections during execution
    - _Requirements: 8.2, 8.4_
  
  - [ ] 15.3 Write property test for executor reading connections
    - **Property 17: Executor reads from connections collection**
    - **Validates: Requirements 8.1, 8.3**
  
  - [ ] 15.4 Write property test for data propagation
    - **Property 18: Executor propagates data correctly**
    - **Validates: Requirements 8.2, 8.4**

- [ ] 16. Add data consistency validation
  - [x] 16.1 Implement connection validation function
    - Check all connections reference valid pin IDs
    - Check all pins belong to existing nodes
    - Check pin directions are compatible
    - Run after any operation that modifies connections
    - _Requirements: 11.1, 11.2, 11.4_
  
  - [ ] 16.2 Write property test for connection validity
    - **Property 24: Connections reference valid pins**
    - **Validates: Requirements 11.1, 11.2**
  
  - [ ] 16.3 Write property test for pin direction validation
    - **Property 25: Pin direction validation**
    - **Validates: Requirements 11.4**

- [ ] 17. Add runtime links read-only enforcement
  - [ ] 17.1 Add tests to verify Pin.links modifications don't affect connections
    - Modify Pin.links at runtime
    - Verify connections array unchanged
    - Verify serialization uses connections array
    - _Requirements: 12.4_
  
  - [ ] 17.2 Write property test for runtime links read-only
    - **Property 28: Runtime links are read-only**
    - **Validates: Requirements 12.4**

- [ ] 18. Final checkpoint - Comprehensive testing
  - Run all unit tests
  - Run all property-based tests
  - Test copy/paste with various selections
  - Test undo/redo with connection operations
  - Test graph execution with connections
  - Verify serialization/deserialization round-trips
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 19. Integration testing and cleanup
  - [ ] 19.1 Test frontend-backend round-trip
    - Create graph in frontend
    - Serialize and send to backend
    - Backend deserializes and re-serializes
    - Frontend deserializes response
    - Verify equivalence
    - _Requirements: 2.4, 10.4_
  
  - [ ] 19.2 Test copy/paste across subgraphs
    - Copy nodes from one subgraph
    - Paste into different subgraph
    - Verify connections filtered correctly
    - Verify new IDs generated
    - _Requirements: 3.1, 3.2, 3.3, 3.4_
  
  - [ ] 19.3 Remove any legacy Pin.links serialization code
    - Clean up old serialization logic
    - Remove unused imports
    - Update comments and documentation
  
  - [ ] 19.4 Write integration tests for complete workflows
    - Test create → serialize → deserialize → execute
    - Test copy → paste → execute
    - Test create → undo → redo → execute

## Notes

- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The refactor is designed to be completed in phases, with each phase building on the previous
- Backend changes should be completed before frontend integration to ensure API stability
- Runtime Pin.links synchronization is critical for maintaining consistency during editing
