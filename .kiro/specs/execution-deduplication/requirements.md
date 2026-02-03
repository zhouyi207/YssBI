# Execution Deduplication - Requirements

## Overview
Fix the issue where nodes with multiple exec outputs connecting to the same downstream node cause that node to be executed multiple times within a single execution frame, leading to freezes and incorrect behavior.

## Problem Statement
Currently, when a Sequence node (or any multi-output node) has multiple exec outputs connected to the same downstream node's input, the downstream node gets executed once for each connection. This causes:

1. **Duplicate execution**: The same node runs multiple times in quick succession
2. **Resource exhaustion**: Multiple window creations or heavy operations
3. **Unexpected behavior**: Nodes designed to run once per frame run multiple times
4. **False cycle detection**: The execution stack may incorrectly flag cycles

### Example Problematic Graph
```
Event → Sequence1
  Sequence1.Then0 → Sequence2.In
  Sequence1.Then1 → Sequence2.In
```

Expected: Sequence2 executes once
Actual: Sequence2 executes twice (once from Then0, once from Then1)

## User Stories

### 1. As a user, I want Sequence nodes to work correctly when chained
**Acceptance Criteria:**
- 1.1 When multiple exec outputs from the same node connect to the same downstream input, the downstream node should execute only once per execution frame
- 1.2 The execution order should be deterministic and predictable
- 1.3 No false cycle detection should occur for valid multi-output patterns
- 1.4 The fix should work for all multi-output control flow nodes (Sequence, Sequence5, etc.)

### 2. As a user, I want Plot nodes to open only once when triggered by multiple connections
**Acceptance Criteria:**
- 2.1 When a Plot node is connected to multiple outputs from a Sequence node, it should open only one window
- 2.2 Window creation should not cause application freezes
- 2.3 The behavior should be consistent across all visualization nodes

### 3. As a developer, I want the execution system to handle multi-output patterns correctly
**Acceptance Criteria:**
- 3.1 The execution context should track which nodes have been executed in the current frame
- 3.2 Nodes should be deduplicated before execution
- 3.3 The deduplication should reset between execution frames
- 3.4 The solution should not break existing single-connection patterns

## Technical Requirements

### 1. Execution Frame Tracking
- Track which nodes have been executed in the current execution frame
- Reset the tracking when a new execution frame starts
- Use a per-frame execution set to prevent duplicate execution

### 2. Deduplication Logic
- Before executing a node via `execute_pin_downstream`, check if it has already been executed in this frame
- Skip execution if the node was already executed
- Log deduplication events for debugging

### 3. Backward Compatibility
- Ensure existing graphs with single connections continue to work
- Do not break data flow evaluation
- Maintain correct execution order for sequential operations

## Non-Functional Requirements

### Performance
- Deduplication check should be O(1) using a HashSet
- Minimal overhead for single-connection cases
- No memory leaks from tracking data

### Reliability
- No false positives (skipping nodes that should execute)
- No false negatives (executing nodes that should be skipped)
- Proper cleanup between execution frames

## Out of Scope
- Parallel execution of multiple branches
- Cross-frame execution tracking
- Data flow deduplication (only exec flow)

## Success Metrics
1. The provided test graph (Event → Sequence1 → Sequence2) executes without freezing
2. Sequence2 executes exactly once, not twice
3. All existing tests continue to pass
4. No performance regression in execution speed
