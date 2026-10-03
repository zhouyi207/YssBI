import { expect, it } from "vitest";
import { parseHarnessEvent } from "@/services/assistant/harnessContract";
import {
  INITIAL_SNAPSHOT,
  reduceHarnessEvent,
  type AssistantHarnessSnapshot,
} from "./assistantHarnessProjection";

function apply(snapshot: AssistantHarnessSnapshot, type: string, payload?: unknown) {
  return reduceHarnessEvent(
    snapshot,
    parseHarnessEvent({
      sessionId: "session-1",
      sequence: snapshot.lastSequence + 1,
      turnId: "turn-1",
      occurredAt: 1000,
      type,
      ...(payload ? { payload } : {}),
    }),
  );
}

it("preserves evidence, tools and other messages when a text delta changes one part", () => {
  let snapshot = apply(INITIAL_SNAPSHOT, "turn_started", { userMessage: "Inspect" });
  snapshot = apply(snapshot, "plan_proposed", { plan: { researchQuestion: "Inspect" } });
  snapshot = apply(snapshot, "tool_invocation_started", {
    invocationId: "tool-1",
    capabilityId: "inspect_graph",
  });
  snapshot = apply(snapshot, "text_delta", { delta: "Found" });
  const previous = snapshot;
  snapshot = apply(snapshot, "text_delta", { delta: " two columns" });
  expect(snapshot.messages[0]).toBe(previous.messages[0]);
  expect(snapshot.messages[1].content[0]).toBe(previous.messages[1].content[0]);
  expect(snapshot.messages[1].content[1]).toBe(previous.messages[1].content[1]);
  expect(snapshot.messages[1].content[2]).toEqual({ type: "text", text: "Found two columns" });
  expect(previous.messages[1].content[2]).toEqual({ type: "text", text: "Found" });
});

it("settles only unfinished tools and keeps completed evidence and streamed text in place", () => {
  let snapshot = apply(INITIAL_SNAPSHOT, "turn_started", { userMessage: "Inspect" });
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
