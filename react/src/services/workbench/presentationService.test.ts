import { beforeEach, expect, it, vi } from "vitest";
import { subscribeUiIntents } from "./presentationService";
import { parseUiIntent } from "@/shared/types/domain/uiPresentation";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage?: (value: unknown) => void;
  },
}));
vi.mock("@/services/ipc", () => ({ invokeCommand: invoke }));
vi.mock("@/services/devHmrIpc", () => ({
  trackChannel: (channel: unknown) => channel,
  untrackChannel: vi.fn(),
}));

beforeEach(() => invoke.mockReset());

it("validates typed resource opening and restricts node focus to node graphs", () => {
  const intent = {
    kind: "openResource",
    resource: { kind: "doc", id: "docs/Report.md" },
    nodeId: null,
  };
  expect(parseUiIntent(intent)).toEqual(intent);
  expect(() =>
    parseUiIntent({ ...intent, resource: { ...intent.resource, kind: "unknown" } }),
  ).toThrow();
  expect(() =>
    parseUiIntent({ ...intent, nodeId: "00000000-0000-0000-0000-000000000001" }),
  ).toThrow();
  expect(
    parseUiIntent({
      ...intent,
      resource: { kind: "function_graph", id: "functions/F.yssbi-function" },
      nodeId: "00000000-0000-0000-0000-000000000001",
    }),
  ).toMatchObject({ kind: "openResource" });
});

it("delivers validated workbench intents and releases the subscription once", async () => {
  invoke.mockResolvedValue("subscription");
  const onEvent = vi.fn();
  const onError = vi.fn();
  const close = await subscribeUiIntents("project", onEvent, onError);
  expect(invoke).toHaveBeenCalledWith("subscribe_ui_intents", {
    projectInstanceId: "project",
    channel: expect.anything(),
  });
  const deliver = invoke.mock.calls[0][1].channel.onmessage;
  const event = {
    kind: "intent",
    receipt: {
      id: "00000000-0000-0000-0000-000000000001",
      intent: { kind: "showPanel", panel: "details" },
      status: "pending",
    },
  };
  deliver(event);
  expect(onEvent).toHaveBeenCalledWith(event);
  deliver({ ...event, receipt: { ...event.receipt, status: "invalid" } });
  expect(onError).toHaveBeenCalledTimes(1);
  await close();
  await close();
  expect(
    invoke.mock.calls.filter(([command]) => command === "unsubscribe_ui_intents"),
  ).toHaveLength(1);
  deliver(event);
  expect(onEvent).toHaveBeenCalledTimes(1);
});
