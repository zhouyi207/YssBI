# Pin User Value Fix - Complete

## Problem Summary

When users set a value on a Pin (e.g., Print node's Value pin), the value was not being used during execution, resulting in `null` output.

## Root Cause Analysis

The issue had multiple layers:

1. **Critical: Missing userValue in Serialization** ⚠️
   - The `serializeSubGraph` function in `io.ts` was NOT including `userValue` when serializing pins
   - Even though the frontend store was updated with userValue, it was lost during serialization
   - When execution happened, the serialized data sent to backend didn't include userValue

2. **Backend State vs Frontend State Mismatch**:
   - The `PinInput` component was calling `update_pin_user_value` to update the backend state
   - However, the frontend store was not being updated with this change
   - When execution happened via `executeProject`, it sent the frontend store data to the backend

3. **Missing Type Definition**:
   - The `Pin` interface in TypeScript was missing the `userValue` field
   - This prevented proper type checking and IDE support

## Solution Implemented

### 1. **CRITICAL FIX**: Serialize userValue in io.ts

**File**: `src/components/Editor/Utils/io.ts`

Added `userValue` to the pin serialization:

```typescript
inputs: node.inputs.map((p) => ({
  id: p.id,
  name: p.name,
  type: p.type,
  links: p.links,
  defaultValue: p.defaultValue,
  userValue: p.userValue,  // 🆕 CRITICAL: Include user value
  isArray: p.isArray,
})),
outputs: node.outputs.map((p) => ({
  id: p.id,
  name: p.name,
  type: p.type,
  links: p.links,
  defaultValue: p.defaultValue,
  userValue: p.userValue,  // 🆕 CRITICAL: Include user value
  isArray: p.isArray,
})),
```

### 2. Frontend Type System Update

**File**: `src/components/Editor/Types/nodes.ts`

Added `userValue` field to the `Pin` interface:

```typescript
export interface Pin {
  id: string;
  nodeId: string;
  name: string;
  type: PinType;
  direction: PinDirection;
  links: string[];
  defaultValue?: any;
  userValue?: any;      // 🆕 User-set value (overrides default)
  isArray?: boolean;
  ui?: { ... };
}
```

### 3. PinInput Component Update

**File**: `src/components/Editor/Pins/PinInput.tsx`

Updated the component to:
1. Import `useNodeStore` to access the frontend state
2. Update both backend AND frontend store when value changes
3. Apply the same logic for all input types (text, number, boolean)

Key changes:
```typescript
import { useNodeStore } from "../Store/useNodeStore";

const updateNode = useNodeStore((state) => state.updateNode);

const handleBlur = useCallback(async () => {
  // ... existing code ...
  
  // Save to backend
  await invoke("update_pin_user_value", {
    subgraphId,
    nodeId,
    pinId,
    value,
  });
  
  // 🆕 Update frontend store
  updateNode(subgraphId, nodeId, (node) => {
    const cloned = node.clone();
    const inputPin = cloned.inputs.find((p) => p.id === pinId);
    if (inputPin) {
      inputPin.userValue = value;
    }
    return cloned;
  });
}, [subgraphId, nodeId, pinId, value, updateNode]);
```

### 4. Pin Component Update

**File**: `src/components/Editor/Pins/Pin.tsx`

Updated to:
1. Accept `userValue` from props
2. Pass `userValue ?? defaultValue` to PinInput (priority: userValue > defaultValue)

```typescript
export const Pin: React.FC<PinProps> = (props) => {
  const {
    // ... other props ...
    defaultValue,
    userValue,  // 🆕 Added
    // ...
  } = props;
  
  // ...
  
  <PinInput
    // ... other props ...
    value={userValue ?? defaultValue}  // 🆕 Priority: userValue > defaultValue
  />
}
```

## Value Priority System

The system now implements a three-tier priority for pin values:

1. **Connected Value** (Highest Priority): If the pin is connected to another pin, use the value from that connection
2. **User Value** (Medium Priority): If no connection, use the value set by the user via the input widget
3. **Default Value** (Lowest Priority): If no connection and no user value, use the default value from the node definition

This is implemented in:
- **Frontend**: `Pin.tsx` passes `userValue ?? defaultValue` to `PinInput`
- **Frontend Serialization**: `io.ts` includes `userValue` when serializing nodes
- **Backend**: `ExecutionContext::get_pin_value()` checks in order: connection → user_value → default_value

## Data Flow

```
User Input → PinInput Component
    ↓
    ├─→ Backend State (update_pin_user_value command)
    └─→ Frontend Store (useNodeStore.updateNode)
         ↓
         Serialization (serializeSubGraph with userValue) ✅
         ↓
         Execute (executeProject sends complete data)
         ↓
         Backend Execution (uses userValue from data)
         ↓
         Output (correct value, not null)
```

## Testing

To test the fix:

1. Open a project with an Event
2. Add a Print node
3. Click on the Print node's "Value" pin input widget
4. Enter a value (e.g., "Hello World")
5. Press Enter or click away to save
6. Execute the Event
7. The Print node should now output the entered value instead of `null`

## Files Modified

1. `src/components/Editor/Utils/io.ts` - **CRITICAL**: Added userValue to serialization
2. `src/components/Editor/Types/nodes.ts` - Added `userValue` field to Pin interface
3. `src/components/Editor/Pins/PinInput.tsx` - Updated to sync with frontend store
4. `src/components/Editor/Pins/Pin.tsx` - Updated to use userValue with priority

## Backend Files (Already Correct)

The following backend files were already correctly implemented:

1. `src-tauri/src/commands/nodes.rs` - `update_pin_user_value` command
2. `src-tauri/src/executor/context.rs` - `get_pin_value` with priority logic
3. `src-tauri/src/project/dto.rs` - `PinDto` with `user_value` field
4. `src-tauri/src/project/mod.rs` - `SerializedPin` with `user_value` field

## Notes

- The fix ensures that userValue is preserved through the entire data flow
- When the project is saved, the `userValue` will be persisted to the project file
- The execution now uses the correct data with user values included
- TypeScript type safety is maintained throughout the system
