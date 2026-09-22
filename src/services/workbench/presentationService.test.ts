import { expect, it, vi } from "vitest";
import { subscribeUi } from "./presentationService";

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
