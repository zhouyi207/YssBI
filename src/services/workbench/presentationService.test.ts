import { beforeEach, expect, it, vi } from "vitest";
import { activateUiElement, readUiPage, subscribeUi, updateUiPage } from "./presentationService";
import type { ResultDescriptor } from "@/shared/types/domain/result";
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

it("shares one project stream, recovers a late listener and releases only after the final listener", async () => {
  invoke.mockResolvedValue("subscription");
  const first = vi.fn();
  const second = vi.fn();
  const fail = vi.fn();
  const closeFirst = await subscribeUi("project", false, first, fail);
  const closeSecond = await subscribeUi("project", false, second, fail);
  expect(invoke.mock.calls.filter(([command]) => command === "subscribe_ui")).toHaveLength(1);
  expect(second).toHaveBeenCalledWith({ kind: "resync" });
  const channel = invoke.mock.calls[0][1].channel;
  await closeFirst();
  expect(invoke.mock.calls.filter(([command]) => command === "unsubscribe_ui")).toHaveLength(0);
  first.mockClear();
  channel.onmessage({ kind: "resync" });
  expect(first).not.toHaveBeenCalled();
  expect(second).toHaveBeenCalledTimes(2);
  await closeSecond();
  expect(invoke).toHaveBeenLastCalledWith("unsubscribe_ui", { subscriptionId: "subscription" });
  expect(fail).not.toHaveBeenCalled();
});

it("projects full result descriptors to the closed source contract for every page command", async () => {
  const descriptor: ResultDescriptor = {
    executionSessionId: "00000000-0000-0000-0000-000000000001",
    resultId: "42",
    provenance: {
      runId: "run",
      graphPath: "graphs/report.yssbi",
      nodeId: "00000000-0000-0000-0000-000000000002",
      output: null,
      createdAtMs: "123",
    },
    presentation: { kind: "report", report: "structured" },
    valueKind: "scalar",
    metadata: null,
    totalCount: null,
    title: "Report",
  };
  const original = structuredClone(descriptor);
  const source = {
    executionSessionId: descriptor.executionSessionId,
    resultId: descriptor.resultId,
  };
  const page = {
    source,
    revision: 1,
    spec: {
      root: "report",
      elements: {
        report: {
          component: { type: "text", props: { text: "Report" } },
          visible: true,
          children: [],
        },
      },
    },
  };
  const update = { kind: "snapshot", page };
  const receipt = {
    id: "00000000-0000-0000-0000-000000000003",
    intent: { kind: "showPanel", panel: "assistant" },
    status: "pending",
  };
  invoke.mockResolvedValueOnce({ kind: "page", page });
  expect(await readUiPage("project", descriptor)).toEqual(page);
  expect(invoke).toHaveBeenLastCalledWith("inspect_ui", {
    projectInstanceId: "project",
    request: { kind: "page", source },
  });

  invoke.mockResolvedValueOnce(update);
  expect(await updateUiPage("project", descriptor, 1, { kind: "reset" })).toEqual(update);
  expect(invoke).toHaveBeenLastCalledWith("update_ui", {
    projectInstanceId: "project",
    request: { source, baseRevision: 1, action: { kind: "reset" } },
  });

  invoke.mockResolvedValueOnce(receipt);
  expect(await activateUiElement("project", descriptor, 1, "assistant")).toEqual(receipt);
  expect(invoke).toHaveBeenLastCalledWith("activate_ui_element", {
    projectInstanceId: "project",
    request: { source, baseRevision: 1, id: "assistant", clientKey: expect.any(String) },
  });
  expect(descriptor).toEqual(original);
});
