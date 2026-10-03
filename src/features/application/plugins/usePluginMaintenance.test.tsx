// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { usePluginMaintenance } from "./usePluginMaintenance";
import type { PluginSnapshot } from "./pluginRegistry";

const service = vi.hoisted(() => ({
  history: vi.fn(),
  storage: vi.fn(),
  diagnostics: vi.fn(),
  clearHistory: vi.fn(),
  clearCache: vi.fn(),
  collectGarbage: vi.fn(),
}));
vi.mock("@/services/plugins/pluginService", () => ({ pluginService: service }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
const plugin = {
  manifest: { id: "example.plugin" },
  installationGeneration: "1",
} as PluginSnapshot;
const firstPage = { tasks: [{ taskId: "first" }], nextCursor: "next" };
let root: Root;
let current: ReturnType<typeof usePluginMaintenance>;
function Harness({ value = plugin }: { value?: PluginSnapshot }) {
  current = usePluginMaintenance(value);
  return null;
}

beforeEach(async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.resetAllMocks();
  service.history.mockResolvedValue(firstPage);
  service.storage.mockResolvedValue({ pluginId: plugin.manifest.id });
  service.diagnostics.mockResolvedValue([]);
  root = createRoot(document.createElement("div"));
  await act(async () => root.render(<Harness />));
});
afterEach(async () => {
  await act(async () => root.unmount());
  vi.unstubAllGlobals();
});

it("admits one maintenance operation until its refresh has settled", async () => {
  const cleaned = deferred<void>();
  service.clearCache.mockReturnValueOnce(cleaned.promise);
  let clearing!: Promise<void>;
  let duplicate!: Promise<void>;
  let reading!: Promise<void>;
  act(() => {
    clearing = current.clearCache();
    duplicate = current.clearHistory();
    reading = current.more();
  });
  expect(service.clearCache).toHaveBeenCalledOnce();
  expect(service.clearHistory).not.toHaveBeenCalled();
  expect(service.history).toHaveBeenCalledOnce();
  const reloaded = deferred<typeof firstPage>();
  service.history.mockReturnValueOnce(reloaded.promise);
  await act(async () => {
    cleaned.resolve();
  });
  expect(current.busy).toBe(true);
  expect(service.history).toHaveBeenCalledTimes(2);
  await act(async () => {
    await current.clearHistory();
  });
  expect(service.clearHistory).not.toHaveBeenCalled();
  await act(async () => {
    reloaded.resolve(firstPage);
    await Promise.all([clearing, duplicate, reading]);
  });
  expect(current.busy).toBe(false);
});

it("preserves pages for the same installation and rejects work from a replaced identity", async () => {
  service.history.mockResolvedValueOnce({ tasks: [{ taskId: "second" }], nextCursor: null });
  await act(async () => current.more());
  const tasks = current.tasks;
  await act(async () => root.render(<Harness value={{ ...plugin }} />));
  expect(service.history).toHaveBeenCalledTimes(2);
  expect(current.tasks).toBe(tasks);

  const oldPage = deferred<typeof firstPage>();
  service.history.mockReturnValueOnce(oldPage.promise);
  let oldRefresh!: Promise<void>;
  act(() => {
    oldRefresh = current.refresh();
  });
  await act(async () =>
    root.render(<Harness value={{ ...plugin, installationGeneration: "2" }} />),
  );
  const replacementTasks = current.tasks;
  await act(async () => {
    oldPage.resolve({ tasks: [{ taskId: "obsolete" }], nextCursor: "old" });
    await oldRefresh;
  });
  expect(current.tasks).toBe(replacementTasks);
  expect(current.tasks.map((task) => task.taskId)).toEqual(["first"]);
  expect(current.busy).toBe(false);
});
