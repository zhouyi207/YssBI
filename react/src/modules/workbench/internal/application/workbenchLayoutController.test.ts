import { Actions } from "flexlayout-react";
import { expect, it, vi } from "vitest";
import { logDomainPanelId } from "@/features/domain/log/logDomains";
import { LayoutModelBinding } from "../layout/layoutModelBinding";
import { createDefaultLogsLayout } from "../layout/logsLayoutModel";
import { createLogsLayoutRuntime } from "../layout/logsRuntime";
import { configureWorkbenchModel } from "../layout/workbenchActivityGroup";
import { createEmptyWorkbenchLayout } from "../layout/workbenchLayoutDefaults";
import { createWorkbenchLayoutRuntime } from "../layout/workbenchLayoutInternal";
import { WorkbenchModelOperations } from "../layout/workbenchLayoutOperations";
import { workbenchLayoutStorageKey } from "../layout/workbenchLayoutPersistence";
import { createWorkbenchLayoutController } from "./workbenchLayoutController";

function createBinding() {
  return new LayoutModelBinding(createEmptyWorkbenchLayout(), configureWorkbenchModel);
}

function createFixture() {
  const runtime = createWorkbenchLayoutRuntime();
  const logs = createLogsLayoutRuntime();
  const logsBinding = new LayoutModelBinding(createDefaultLogsLayout());
  logs.bind(logsBinding);
  const stored = new Map<string, string>();
  const controller = createWorkbenchLayoutController({
    layoutRead: runtime.read,
    layoutControl: runtime.control,
    internal: runtime.internal,
    logsRead: logs,
    logsControl: logs,
    storage: {
      getItem: (key) => stored.get(key) ?? null,
      setItem: (key, value) => {
        stored.set(key, value);
      },
    },
    debounceMs: 10,
  });
  return { runtime, logs, logsBinding, stored, controller };
}

function renameDetails(binding: LayoutModelBinding, title: string) {
  const panel = new WorkbenchModelOperations(binding.getModel())
    .listPanels()
    .find((panel) => panel.metadata.role === "view" && panel.metadata.viewId === "details");
  if (!panel) throw new Error("Expected hydrated Details panel");
  binding.getModel().doAction(Actions.updateNodeAttributes(panel.panelInstanceId, { name: title }));
}

it("preserves successor hydration and persistence when final Logs capture rebinds the root", async () => {
  vi.useFakeTimers();
  const { runtime, logs, logsBinding, stored, controller } = createFixture();
  const previous = createBinding();
  const successor = createBinding();
  let requested: { binding: LayoutModelBinding; label: string } | undefined;
  const stop = logs.subscribe(() => {
    const next = requested;
    if (!next) return;
    requested = undefined;
    controller.bind(next.binding, next.label);
  });
  try {
    controller.bind(previous, "previous");
    await controller.whenHydrated();
    requested = { binding: successor, label: "successor" };
    logsBinding.getModel().doAction(
      Actions.updateNodeAttributes(logDomainPanelId("all"), {
        name: "Unbind capture",
      }).setAdjusting(true),
    );
    controller.unbind(previous);
    await controller.whenHydrated();
    expect(runtime.read.isHydrated).toBe(true);
    renameDetails(successor, "Successor is live");
    await vi.advanceTimersByTimeAsync(10);
    expect(stored.get(workbenchLayoutStorageKey("successor"))).toContain("Successor is live");

    const outer = createBinding();
    const latest = createBinding();
    requested = { binding: latest, label: "latest" };
    logsBinding.getModel().doAction(
      Actions.updateNodeAttributes(logDomainPanelId("all"), {
        name: "Bind handoff",
      }).setAdjusting(true),
    );
    controller.bind(outer, "outer");
    await controller.whenHydrated();
    renameDetails(latest, "Latest is live");
    await vi.advanceTimersByTimeAsync(10);
    expect(stored.get(workbenchLayoutStorageKey("latest"))).toContain("Latest is live");
    expect(stored.has(workbenchLayoutStorageKey("outer"))).toBe(false);
  } finally {
    stop();
    controller.unbind();
    logs.unbind();
    vi.useRealTimers();
  }
});

it("does not suspend or cancel successor writes while an old close flush awaits idle", async () => {
  vi.useFakeTimers();
  const { runtime, logs, stored, controller } = createFixture();
  const previous = createBinding();
  const successor = createBinding();
  let releaseIdle!: () => void;
  const idle = new Promise<void>((resolve) => {
    releaseIdle = resolve;
  });
  const whenIdle = vi.spyOn(runtime.internal, "whenIdle").mockReturnValueOnce(idle);
  let flushing: Promise<void> | undefined;
  try {
    controller.bind(previous, "previous");
    await controller.whenHydrated();
    flushing = controller.flushBeforeWindowClose();
    await Promise.resolve();
    expect(whenIdle).toHaveBeenCalledOnce();

    controller.bind(successor, "successor");
    await controller.whenHydrated();
    renameDetails(successor, "Write while old flush waits");
    await vi.advanceTimersByTimeAsync(10);
    expect(stored.get(workbenchLayoutStorageKey("successor"))).toContain(
      "Write while old flush waits",
    );

    renameDetails(successor, "Write survives old finally");
    releaseIdle();
    await flushing;
    await vi.advanceTimersByTimeAsync(10);
    expect(stored.get(workbenchLayoutStorageKey("successor"))).toContain(
      "Write survives old finally",
    );
  } finally {
    releaseIdle();
    await flushing;
    whenIdle.mockRestore();
    controller.unbind();
    logs.unbind();
    vi.useRealTimers();
  }
});
