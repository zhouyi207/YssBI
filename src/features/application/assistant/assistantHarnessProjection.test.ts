import { testModelIdentity } from "@/tests/fixtures/harnessModels";
import { expect, it } from "vitest";
import { parseHarnessEvent } from "@/services/assistant/harnessContract";
import type { ProjectionAgentTask } from "./assistantMessageContent";
import {
  INITIAL_SNAPSHOT,
  reduceHarnessEvent,
  type AssistantHarnessSnapshot,
} from "./assistantHarnessProjection";

function apply(
  snapshot: AssistantHarnessSnapshot,
  type: string,
  payload?: unknown,
  occurredAt = 1000,
) {
  return reduceHarnessEvent(
    snapshot,
    parseHarnessEvent({
      sessionId: "session-1",
      sequence: snapshot.lastSequence + 1,
      turnId: "turn-1",
      occurredAt,
      type,
      ...(payload ? { payload } : {}),
    }),
  );
}

it("retains elapsed boundaries for completed tools, resumed worker attempts and cancelled turns", () => {
  let snapshot = apply(
    INITIAL_SNAPSHOT,
    "turn_started",
    { model: testModelIdentity, resources: [], userMessage: "Build report" },
    1000,
  );
  expect(snapshot.messages[1].createdAt.getTime()).toBe(1000);
  snapshot = apply(
    snapshot,
    "tool_invocation_started",
    { invocationId: "inspect", capabilityId: "inspect_graph" },
    2000,
  );
  snapshot = apply(
    snapshot,
    "tool_invocation_completed",
    { invocationId: "inspect", capabilityId: "inspect_graph" },
    2500,
  );
  snapshot = apply(
    snapshot,
    "agent_run_started",
    { runId: "report", parentRunId: "manager", role: "report", objective: "Write report" },
    3000,
  );
  snapshot = apply(
    snapshot,
    "agent_run_output",
    {
      runId: "report",
      event: {
        type: "tool_invocation_started",
        payload: { invocationId: "write", capabilityId: "edit_resource" },
      },
    },
    4000,
  );
  snapshot = apply(
    snapshot,
    "agent_run_output",
    {
      runId: "report",
      event: {
        type: "tool_invocation_started",
        payload: { invocationId: "read", capabilityId: "inspect_resource" },
      },
    },
    5000,
  );
  snapshot = apply(
    snapshot,
    "agent_run_output",
    {
      runId: "report",
      event: {
        type: "tool_invocation_failed",
        payload: {
          invocationId: "read",
          capabilityId: "inspect_resource",
          failureCode: "deadline_elapsed",
        },
      },
    },
    6000,
  );
  snapshot = apply(
    snapshot,
    "agent_run_finished",
    {
      runId: "report",
      role: "report",
      state: "failed",
      failureCode: "provider_transport_failed",
      summary: null,
      blockedReason: null,
      warnings: [],
      evidenceCount: 0,
      artifacts: [],
      results: [],
    },
    7000,
  );
  snapshot = apply(
    snapshot,
    "agent_run_resumed",
    { runId: "report", role: "report", objective: "Finish report" },
    11000,
  );
  snapshot = apply(snapshot, "turn_cancelled", undefined, 16000);
  const message = snapshot.messages[1];
  expect(message.finishedAt! - message.createdAt.getTime()).toBe(15000);
  expect(message.content[0]).toMatchObject({ timing: { startedAt: 2000, completedAt: 2500 } });
  const attempts = message.content
    .filter((part) => part.type === "data" && part.name === "agent-task")
    .map((part) => (part as { data: ProjectionAgentTask }).data);
  expect(attempts.map((task) => [task.state, task.finishedAt! - task.startedAt])).toEqual([
    ["failed", 4000],
    ["cancelled", 5000],
  ]);
  expect(attempts[0].tools[0]).toMatchObject({
    timing: { startedAt: 4000, completedAt: 7000 },
    result: { status: "interrupted" },
  });
  expect(attempts[0].tools[1]).toMatchObject({
    timing: { startedAt: 5000, completedAt: 6000 },
    result: { status: "timed-out", failureCode: "deadline_elapsed" },
  });
  snapshot = reduceHarnessEvent(
    snapshot,
    parseHarnessEvent({
      sessionId: "session-1",
      sequence: snapshot.lastSequence + 1,
      turnId: "turn-2",
      occurredAt: 30000,
      type: "turn_started",
      payload: { model: testModelIdentity, resources: [], userMessage: "Next task" },
    }),
  );
  expect(snapshot.messages[1]).toBe(message);
});

it("preserves evidence, tools and other messages when a text delta changes one part", () => {
  let snapshot = apply(INITIAL_SNAPSHOT, "turn_started", {
    model: testModelIdentity,
    resources: [],
    userMessage: "Inspect",
  });
  snapshot = apply(snapshot, "plan_proposed", { plan: { researchQuestion: "Inspect" } });
  snapshot = apply(snapshot, "tool_invocation_started", {
    invocationId: "tool-1",
    capabilityId: "inspect_graph",
  });
  const citation = {
    sourceId: "statistics",
    documentId: "regression",
    chunkId: "regression-assumptions",
    title: "Regression assumptions",
    version: "1",
    sourceHash: "source-hash",
  };
  snapshot = apply(snapshot, "agent_run_output", {
    runId: "review",
    event: { type: "knowledge_cited", payload: { citation } },
  });
  snapshot = apply(snapshot, "knowledge_cited", { citation });
  snapshot = apply(snapshot, "text_delta", { delta: "Found" });
  const previous = snapshot;
  snapshot = apply(snapshot, "text_delta", { delta: " two columns" });
  expect(snapshot.messages[0]).toBe(previous.messages[0]);
  expect(snapshot.messages[1].content[0]).toBe(previous.messages[1].content[0]);
  expect(snapshot.messages[1].content[1]).toBe(previous.messages[1].content[1]);
  expect(snapshot.messages[1].content[2]).toBe(previous.messages[1].content[2]);
  expect(snapshot.messages[1].content[2]).toMatchObject({ type: "source", id: citation.chunkId });
  expect(snapshot.messages[1].content[3]).toEqual({ type: "text", text: "Found two columns" });
  expect(previous.messages[1].content[3]).toEqual({ type: "text", text: "Found" });
});

it("settles only unfinished tools and keeps completed evidence and streamed text in place", () => {
  let snapshot = apply(INITIAL_SNAPSHOT, "turn_started", {
    model: testModelIdentity,
    resources: [],
    userMessage: "Inspect",
  });
  snapshot = apply(snapshot, "tool_invocation_started", {
    invocationId: "done",
    capabilityId: "inspect_graph",
  });
  snapshot = apply(snapshot, "tool_invocation_completed", {
    invocationId: "done",
    capabilityId: "inspect_graph",
  });
  snapshot = apply(snapshot, "text_delta", { delta: "Graph inspected" });
  snapshot = apply(snapshot, "tool_invocation_started", {
    invocationId: "pending",
    capabilityId: "inspect_graph",
  });
  const completed = snapshot.messages[1].content[0];
  snapshot = apply(snapshot, "turn_cancelled");
  expect(snapshot.messages[1].content[0]).toBe(completed);
  expect(snapshot.messages[1].content).toMatchObject([
    { type: "tool-call", result: { status: "completed" } },
    { type: "text", text: "Graph inspected" },
    { type: "tool-call", result: { status: "cancelled" }, isError: true },
  ]);
  expect(snapshot.messages[1].status).toEqual({ type: "incomplete", reason: "cancelled" });
  expect(snapshot).toMatchObject({ isRunning: false, activity: null });
});

it("replays sampling recovery and keeps exact failure and business delivery states", () => {
  let snapshot = apply(INITIAL_SNAPSHOT, "turn_started", {
    model: testModelIdentity,
    resources: [],
    userMessage: "Create report",
  });
  snapshot = apply(snapshot, "text_delta", { delta: "Saved.半截😀" });
  snapshot = apply(snapshot, "text_retracted", { characters: 3 });
  snapshot = apply(snapshot, "runtime_status", { phase: "reconnecting", attempt: 1 });
  expect(snapshot.activity).toBe("reconnecting");
  snapshot = apply(snapshot, "context_compaction_progress", {
    completedBytes: 60,
    totalBytes: 100,
  });
  expect(snapshot).toMatchObject({ activity: "compacting", compactionProgress: 60 });
  snapshot = apply(snapshot, "context_compacted");
  expect(snapshot).toMatchObject({ activity: null, compactionProgress: null });
  expect(snapshot.messages[1].content).toEqual([{ type: "text", text: "Saved." }]);
  snapshot = apply(snapshot, "tool_invocation_completed", {
    invocationId: "run",
    capabilityId: "execute_graph",
  });
  snapshot = apply(snapshot, "graph_execution_finished", {
    invocationId: "run",
    status: "failed",
    failureCode: "resource_version_changed",
  });
  expect(snapshot.messages[1].content[1]).toMatchObject({
    isError: true,
    result: { executionStatus: "failed", failureCode: "resource_version_changed" },
  });
  snapshot = apply(snapshot, "agent_run_finished", {
    runId: "manager",
    role: "manager",
    state: "failed",
    failureCode: "provider_payment_required",
    summary: null,
    blockedReason: null,
    warnings: [],
    evidenceCount: 0,
    artifacts: [],
    results: [],
  });
  snapshot = apply(snapshot, "turn_failed");
  expect(snapshot.messages[1].content).toContainEqual({
    type: "data",
    name: "assistant-failure",
    data: { code: "provider_payment_required" },
  });
  let blocked = apply(INITIAL_SNAPSHOT, "turn_started", {
    model: testModelIdentity,
    resources: [],
    userMessage: "Create report",
  });
  blocked = apply(blocked, "delivery_blocked", { reason: "report_document_not_saved" });
  blocked = apply(blocked, "turn_completed", { finalText: "Could not save" });
  expect(blocked.messages[1].status).toEqual({ type: "incomplete", reason: "error" });
  blocked = apply(blocked, "agent_run_resumed", {
    runId: "worker",
    role: "stats",
    objective: "Resume analysis",
  });
  blocked = apply(blocked, "agent_run_output", {
    runId: "worker",
    event: {
      type: "graph_execution_finished",
      payload: { invocationId: "worker-run", status: "failed", failureCode: "draft_changed" },
    },
  });
  blocked = apply(blocked, "agent_run_finished", {
    runId: "worker",
    role: "stats",
    state: "completed",
    failureCode: null,
    summary: "Reported the failed execution",
    blockedReason: null,
    warnings: [],
    evidenceCount: 1,
    artifacts: [],
    results: [],
  });
  expect(blocked.messages[1].content).toContainEqual(
    expect.objectContaining({
      name: "agent-task",
      data: expect.objectContaining({ state: "completed", warnings: ["draft_changed"] }),
    }),
  );
});
