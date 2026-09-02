import { describe, expect, it } from "vitest";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { makeProjectedPinData } from "@/tests/helpers/editorProjectionFixtures";
import {
  CONNECTION_SNAP_RADIUS_PX,
  resolveConnectionFeedback,
  resolveConnectionTarget,
} from "./connectionInteraction";

const capability = {
  current: 0,
  maximum: 1,
  ordered: false,
  canAppend: true,
  canReplace: false,
  canMove: false,
};

function pin(partial: Partial<PinData> & Pick<PinData, "id" | "nodeId" | "direction">): PinData {
  return makeProjectedPinData({ ...partial, connections: partial.connections ?? capability });
}

const source = pin({ id: "source", nodeId: "a", direction: "output" });
const target = pin({ id: "target", nodeId: "b", direction: "input" });

describe("connectionInteraction", () => {
  it("uses the locked screen-space snap radius", () => {
    expect(CONNECTION_SNAP_RADIUS_PX).toBe(18);
  });

  it.each([
    ["append", source, target, { kind: "append" }],
    [
      "replace",
      source,
      pin({
        ...target,
        connections: { ...capability, current: 1, canAppend: false, canReplace: true },
      }),
      { kind: "replace", displacedConnectionIds: ["old"] },
    ],
    [
      "full bounded",
      source,
      pin({
        ...target,
        connections: { ...capability, current: 1, canAppend: false, canReplace: false },
      }),
      { kind: "invalid", reason: "capacity" },
    ],
    ["orphan", source, pin({ ...target, orphan: true }), { kind: "invalid", reason: "orphan" }],
    ["same port", source, source, { kind: "invalid", reason: "same-port" }],
    [
      "same node",
      source,
      pin({ ...target, nodeId: source.nodeId }),
      { kind: "invalid", reason: "same-node" },
    ],
    [
      "direction",
      source,
      pin({ ...target, direction: "output" }),
      { kind: "invalid", reason: "same-direction" },
    ],
    [
      "kind",
      source,
      pin({ ...target, type: "exec", kind: "control", dataType: undefined }),
      { kind: "invalid", reason: "kind-mismatch" },
    ],
    [
      "type",
      source,
      pin({
        ...target,
        dataType: { kind: "String" },
        resolvedType: { display: "String", resolved: true, dataType: { kind: "String" } },
      }),
      { kind: "invalid", reason: "type-mismatch" },
    ],
    [
      "control",
      pin({ ...source, type: "exec", kind: "control", dataType: undefined }),
      pin({ ...target, type: "exec", kind: "control", dataType: undefined }),
      { kind: "append" },
    ],
    [
      "effect",
      pin({ ...source, type: "exec", kind: "effect", dataType: undefined }),
      pin({ ...target, type: "exec", kind: "effect", dataType: undefined }),
      { kind: "append" },
    ],
  ] as const)(
    "maps %s compatibility into interaction feedback",
    (_label, candidateSource, candidateTarget, expected) => {
      const ids = expected.kind === "replace" ? { [candidateTarget.id]: ["old"] } : {};
      expect(resolveConnectionFeedback(candidateSource, candidateTarget, ids)).toEqual(expected);
    },
  );

  it("merges, sorts, and deduplicates both occupied endpoint incumbents", () => {
    const occupiedSource = pin({
      ...source,
      connections: { ...capability, current: 2, canAppend: false, canReplace: true },
    });
    const occupiedTarget = pin({
      ...target,
      connections: { ...capability, current: 2, canAppend: false, canReplace: true },
    });
    expect(
      resolveConnectionFeedback(occupiedSource, occupiedTarget, {
        source: ["connection-z", "connection-shared"],
        target: ["connection-a", "connection-shared"],
      }),
    ).toEqual({
      kind: "replace",
      displacedConnectionIds: ["connection-a", "connection-shared", "connection-z"],
    });
  });

  it("snaps to the nearest valid candidate with stable id tie-breaking", () => {
    const result = resolveConnectionTarget({
      source,
      pointer: { x: 10, y: 10 },
      sourceConnectionIds: [],
      candidates: [
        {
          pin: pin({ id: "z", nodeId: "z-node", direction: "input" }),
          center: { x: 12, y: 10 },
          connectionIds: [],
        },
        {
          pin: pin({ id: "a", nodeId: "a-node", direction: "input" }),
          center: { x: 12, y: 10 },
          connectionIds: [],
        },
        {
          pin: pin({ id: "invalid", nodeId: "a", direction: "input" }),
          center: { x: 10, y: 10 },
          connectionIds: [],
        },
      ],
    });
    expect(result.snappedTarget?.id).toBe("a");
    expect(result.feedback).toEqual({ kind: "append" });
  });

  it("hovers invalid candidates inside the radius without snapping them", () => {
    const invalid = pin({ id: "invalid", nodeId: source.nodeId, direction: "input" });
    const result = resolveConnectionTarget({
      source,
      pointer: { x: 0, y: 0 },
      sourceConnectionIds: [],
      candidates: [{ pin: invalid, center: { x: 1, y: 1 }, connectionIds: [] }],
    });
    expect(result.hoveredTarget?.id).toBe("invalid");
    expect(result.snappedTarget).toBeNull();
    expect(result.feedback).toEqual({ kind: "invalid", reason: "same-node" });
  });

  it("returns no hover or feedback when every candidate is outside the explicit radius", () => {
    const result = resolveConnectionTarget({
      source,
      pointer: { x: 100, y: 100 },
      sourceConnectionIds: [],
      candidates: [
        { pin: source, center: { x: 0, y: 0 }, connectionIds: [] },
        {
          pin: pin({ id: "invalid", nodeId: "a", direction: "input" }),
          center: { x: 70, y: 70 },
          connectionIds: [],
        },
      ],
    });

    expect(result).toEqual({
      hoveredTarget: null,
      snappedTarget: null,
      snappedCenter: null,
      feedback: null,
    });
  });

  it("ignores the nearest source candidate when a valid target is within radius", () => {
    const result = resolveConnectionTarget({
      source,
      pointer: { x: 0, y: 0 },
      sourceConnectionIds: [],
      candidates: [
        { pin: source, center: { x: 0, y: 0 }, connectionIds: [] },
        { pin: target, center: { x: 4, y: 0 }, connectionIds: [] },
      ],
    });

    expect(result.hoveredTarget?.id).toBe("target");
    expect(result.snappedTarget?.id).toBe("target");
    expect(result.feedback).toEqual({ kind: "append" });
  });
});
