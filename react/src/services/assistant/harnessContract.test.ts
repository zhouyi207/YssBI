import { isModelConfigured, languageModelCatalogSchema } from "./modelContract";
import modelCatalog from "@/tests/fixtures/node-system-contracts/harness-models.json";
import { testModelCatalog } from "@/tests/fixtures/harnessModels";
import events from "@/tests/fixtures/node-system-contracts/harness-events.json";
import { describe, expect, it } from "vitest";
import {
  InvalidHarnessPayloadError,
  parseHarnessSession,
  parseHarnessEvent,
  parseHarnessTurnResult,
  parseHarnessToolInspection,
} from "./harnessContract";

describe("Harness wire contract", () => {
  it("preserves tool times and exact result identities in inspected and completed output", () => {
    const inspection = {
      target: "docs/report.yssbi-doc",
      parameters: { operation: "edit" },
      artifacts: [
        {
          resource: { kind: "doc", id: "docs/report.yssbi-doc" },
          revision: 7,
          revisionKind: "resource",
          deleted: false,
        },
      ],
      results: [
        {
          executionSessionId: "00000000-0000-0000-0000-000000000001",
          resultId: "18446744073709551615",
          output: "node:result",
        },
      ],
      startedAt: 1000,
      finishedAt: 2500,
      failure: {
        code: "invalid_request",
        details: { category: "missing_field", path: "$.constraints", expected: 'type="string"' },
      },
    };
    expect(parseHarnessToolInspection(inspection)).toEqual(inspection);
    expect(parseHarnessToolInspection({ ...inspection, failure: null }).failure).toBeNull();
    expect(() =>
      parseHarnessToolInspection({
        ...inspection,
        failure: { code: "invalid_request", details: { path: 42 } },
      }),
    ).toThrow(InvalidHarnessPayloadError);
    const finished = events.find((event) => event.type === "agent_run_finished")!;
    const enriched = {
      ...finished,
      payload: {
        ...finished.payload,
        artifacts: inspection.artifacts,
        results: inspection.results,
      },
    };
    expect(parseHarnessEvent(enriched)).toEqual(enriched);
    expect(() =>
      parseHarnessToolInspection({
        ...inspection,
        results: [{ ...inspection.results[0], resultId: 12 }],
      }),
    ).toThrow(InvalidHarnessPayloadError);
  });
  it("accepts the shared Rust lifecycle fixture including recovery and exact failures", () => {
    for (const event of events) expect(parseHarnessEvent(event)).toEqual(event);
    const tool = events.find((event) => event.type === "tool_invocation_started")!;
    for (const capabilityId of [
      "search_knowledge",
      "read_knowledge",
      "delegate_task",
      "followup_task",
      "propose_statistical_plan",
    ]) {
      const event = { ...tool, payload: { ...tool.payload, capabilityId } };
      expect(parseHarnessEvent(event)).toEqual(event);
    }
  });
  it("accepts empty successful turn text in both the receipt and completion event", () => {
    for (const finalText of ["", "Completed"]) {
      const result = { finalText };
      expect(parseHarnessTurnResult(result)).toEqual(result);
      const event = {
        sequence: 1,
        sessionId: "session",
        turnId: "turn",
        occurredAt: 1,
        type: "turn_completed",
        payload: result,
      };
      expect(parseHarnessEvent(event)).toEqual(event);
    }
    for (const finalText of [undefined, null, 0, false, {}]) {
      expect(() => parseHarnessTurnResult({ finalText })).toThrow(InvalidHarnessPayloadError);
    }
  });
  it("parses worker lifecycle and only permits model events inside worker output", () => {
    const base = { sequence: 1, sessionId: "session", turnId: "turn", occurredAt: 1 };
    const started = {
      ...base,
      type: "agent_run_started",
      payload: {
        runId: "worker",
        parentRunId: "manager",
        role: "review",
        objective: "Check the report",
      },
    };
    expect(parseHarnessEvent(started)).toEqual(started);
    const output = {
      ...base,
      type: "agent_run_output",
      payload: {
        runId: "worker",
        event: { type: "text_delta", payload: { delta: "worker progress" } },
      },
    };
    expect(parseHarnessEvent(output).type).toBe("agent_run_output");
    expect(() =>
      parseHarnessEvent({
        ...output,
        payload: {
          ...output.payload,
          event: { type: "turn_completed", payload: { finalText: "false completion" } },
        },
      }),
    ).toThrow(InvalidHarnessPayloadError);
    expect(() =>
      parseHarnessEvent({ ...started, payload: { ...started.payload, role: "unknown" } }),
    ).toThrow(InvalidHarnessPayloadError);
    const finished = {
      ...base,
      type: "agent_run_finished",
      payload: {
        runId: "worker",
        role: "review",
        state: "blocked",
        failureCode: null,
        summary: "Missing results",
        blockedReason: "Evidence unavailable",
        warnings: [],
        evidenceCount: 0,
        artifacts: [],
        results: [],
      },
    };
    expect(parseHarnessEvent(finished)).toEqual(finished);
    expect(() =>
      parseHarnessEvent({ ...finished, payload: { ...finished.payload, state: "running" } }),
    ).toThrow(InvalidHarnessPayloadError);
  });
  it("accepts resource edit lifecycle events without accepting unknown capabilities", () => {
    const event = {
      sequence: 1,
      sessionId: "session",
      turnId: "turn",
      occurredAt: 1,
      type: "tool_invocation_completed",
      payload: { invocationId: "invocation", capabilityId: "edit_resource" },
    };
    expect(parseHarnessEvent(event)).toEqual(event);
    expect(() =>
      parseHarnessEvent({
        ...event,
        payload: { ...event.payload, capabilityId: "execute_arbitrary_code" },
      }),
    ).toThrow(InvalidHarnessPayloadError);
  });
  it("preserves the Rust model catalog and derives availability from its authentication mode", () => {
    expect(languageModelCatalogSchema.parse(testModelCatalog)).toEqual(testModelCatalog);
    const catalog = languageModelCatalogSchema.parse(modelCatalog);
    expect(catalog).toEqual(modelCatalog);
    expect(catalog.providers[0].reasoningDefaults?.["cloud-model"]).toBe("low");
    expect(catalog.providers[1].reasoningDefaults).toBeUndefined();
    expect(isModelConfigured(catalog, catalog.defaultModel)).toBe(true);
    expect(isModelConfigured(catalog, { providerId: "cloud", modelId: "cloud-model" })).toBe(false);
    expect(isModelConfigured(catalog, { providerId: "local", modelId: "missing" })).toBe(false);
    expect(
      languageModelCatalogSchema.safeParse({
        presets: [],
        providers: [{ config: testModelCatalog.providers[0].config }],
        defaultModel: null,
      }).success,
    ).toBe(false);
  });
  it("validates compaction progress without accepting impossible completion counts", () => {
    const event = {
      sequence: 1,
      sessionId: "session",
      turnId: "turn",
      occurredAt: 1,
      type: "context_compaction_progress",
      payload: { completedBytes: 40, totalBytes: 100 },
    };
    expect(parseHarnessEvent(event)).toEqual(event);
    expect(() =>
      parseHarnessEvent({ ...event, payload: { completedBytes: 101, totalBytes: 100 } }),
    ).toThrow(InvalidHarnessPayloadError);
    expect(() =>
      parseHarnessEvent({ ...event, payload: { completedBytes: 0, totalBytes: 0 } }),
    ).toThrow(InvalidHarnessPayloadError);
  });
  it("preserves conversation identity and rejects malformed session metadata", () => {
    const sessions = [
      {
        sessionId: "first",
        projectInstanceId: "prior-runtime",
        projectSessionId: "old-session",
        title: "Previous analysis",
        lastOpenedAt: 2,
        model: null,
      },
      {
        sessionId: "second",
        projectInstanceId: "current-runtime",
        projectSessionId: "current-session",
        title: "",
        lastOpenedAt: 1,
        model: null,
      },
    ];
    expect(sessions.map(parseHarnessSession)).toEqual(sessions);
    expect(() => parseHarnessSession({ ...sessions[0], lastOpenedAt: -1 })).toThrow(
      InvalidHarnessPayloadError,
    );
    expect(() => parseHarnessSession({ sessionId: "missing-metadata" })).toThrow(
      InvalidHarnessPayloadError,
    );
  });
});
