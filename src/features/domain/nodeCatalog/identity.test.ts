import { describe, expect, it } from "vitest";
import {
  BUILTIN_NODE_TYPE_IDS,
  isCallFunctionNodeType,
  isDatabaseResourceNodeType,
  isVariableNodeType,
} from "./identity";

describe("stable node identity", () => {
  it("classifies stable IDs and rejects an unknown ID", () => {
    expect(isCallFunctionNodeType(BUILTIN_NODE_TYPE_IDS.callFunction)).toBe(true);
    expect(isVariableNodeType(BUILTIN_NODE_TYPE_IDS.getVariable)).toBe(true);
    expect(isVariableNodeType(BUILTIN_NODE_TYPE_IDS.setVariable)).toBe(true);
    expect(isDatabaseResourceNodeType(BUILTIN_NODE_TYPE_IDS.getDataframe)).toBe(true);

    const unknownId = "unknown.node-type";
    expect(isCallFunctionNodeType(unknownId)).toBe(false);
    expect(isVariableNodeType(unknownId)).toBe(false);
    expect(isDatabaseResourceNodeType(unknownId)).toBe(false);
  });
});
