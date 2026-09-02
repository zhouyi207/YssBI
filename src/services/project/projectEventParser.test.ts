import { describe, expect, it } from "vitest";
import projectEvents from "@/tests/fixtures/node-system-contracts/project-events.json";
import { parseProjectEvent } from "./projectEventParser";

const projectInstanceId = "00000000-0000-0000-0000-000000000601";
const operationId = "00000000-0000-0000-0000-000000000701";

const lifecycleResult = {
  operationId,
  kind: "saveAs",
  oldProjectInstanceId: null,
  newProjectInstanceId: projectInstanceId,
  phase: "authorityCommitted",
  outcome: "committed",
  record: null,
  path: "D:/projects/copy/metadata.yssbi",
  recovery: null,
  invalidation: { project: true, registry: true },
};

const projectSavedResult = {
  projectInstanceId,
  operationId,
  publicationRevision: 2,
  affectedResources: [{ kind: "chart", key: "charts/Sales.yssbi-chart" }],
  indexInvalidated: true,
  history: { canUndo: true, canRedo: false },
};

const supportedEvents = [
  {
    type: "Project",
    payload: {
      type: "ProjectLoaded",
      payload: {
        result: {
          path: "D:/projects/demo",
          projectInstanceId,
          activationRevision: 7,
        },
      },
    },
  },
  { type: "Project", payload: { type: "ProjectCleared" } },
  {
    type: "Project",
    payload: { type: "ProjectLifecycleCommitted", payload: { result: lifecycleResult } },
  },
  {
    type: "Project",
    payload: { type: "ProjectSaved", payload: { result: projectSavedResult } },
  },
  projectEvents.events[0],
  {
    type: "Resource",
    payload: {
      type: "ProjectIndexInvalidated",
      payload: { projectInstanceId, source: "watcher", version: 9 },
    },
  },
] as const;

describe("project event parser", () => {
  it("unwraps every supported Project and Resource envelope into service-owned events", () => {
    const parsed = supportedEvents.map((value) => parseProjectEvent(value));

    expect(parsed.every((outcome) => outcome.ok)).toBe(true);
    expect(parsed.map((outcome) => outcome.ok && outcome.event.type)).toEqual([
      "ProjectLoaded",
      "ProjectCleared",
      "ProjectLifecycleCommitted",
      "ProjectSaved",
      "ResourceMutationCommitted",
      "ProjectIndexInvalidated",
    ]);
    expect(parsed[1]).toEqual({
      ok: true,
      event: { type: "ProjectCleared", payload: undefined },
    });
    expect(parsed[4]).toEqual({
      ok: true,
      event: {
        type: "ResourceMutationCommitted",
        payload: projectEvents.events[0].payload.payload,
      },
    });
  });

  it("returns only stable parse codes for malformed, unknown, extra, and prose-bearing input", () => {
    const valid = projectEvents.events[0];
    const cases: Array<[unknown, "invalidEnvelope" | "unknownType" | "invalidPayload"]> = [
      [{ ...valid, extra: true }, "invalidEnvelope"],
      [{ ...valid, type: "Unsupported" }, "unknownType"],
      [{ ...valid, payload: { ...valid.payload, extra: true } }, "invalidEnvelope"],
      [{ ...valid, payload: { ...valid.payload, type: "Unsupported" } }, "unknownType"],
      [
        {
          ...valid,
          payload: {
            ...valid.payload,
            payload: { ...valid.payload.payload, extra: true },
          },
        },
        "invalidPayload",
      ],
      [
        {
          type: "Resource",
          payload: {
            type: "ProjectIndexInvalidated",
            payload: { projectInstanceId, source: "watcher", version: 1, message: "backend prose" },
          },
        },
        "invalidPayload",
      ],
      [{ type: "Project", payload: { type: "ProjectCleared", payload: null } }, "invalidEnvelope"],
      [
        {
          code: "backend_failed",
          details: { message: "private backend prose" },
          incidentId: "opaque-1",
        },
        "invalidEnvelope",
      ],
    ];

    for (const [value, code] of cases) {
      const outcome = parseProjectEvent(value);
      expect(outcome).toEqual({ ok: false, code });
      expect(JSON.stringify(outcome)).not.toContain("backend prose");
    }
  });
});
