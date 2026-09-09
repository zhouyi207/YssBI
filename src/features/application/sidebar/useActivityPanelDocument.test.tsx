// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { useActivityPanelDocument } from "./useActivityPanelDocument";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import {
  startProjectLifecycle,
  clearProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { activityPanelFixture } from "@/tests/helpers/activityPanelFixture";
import type { ActivityPanelSnapshot } from "@/shared/types/domain/activityPanel";

const backend = vi.hoisted(() => vi.fn());
vi.mock("@/services/workbench/activityPanelService", () => ({ getActivityPanelDocument: backend }));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ i18n: { language: "en-US" } }) }));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

it("retains the visible tree, coalesces changes and discards replaced or unmounted bindings", async () => {
  const pending: Array<(snapshot: ActivityPanelSnapshot) => void> = [];
  backend.mockImplementation(
    () => new Promise<ActivityPanelSnapshot>((resolve) => pending.push(resolve)),
  );
  let current: ReturnType<typeof useActivityPanelDocument> | undefined;
  function Probe() {
    current = useActivityPanelDocument("project");
    return null;
  }
  const root = createRoot(document.createElement("div"));
  const first = { cursor: "first", document: activityPanelFixture("project", []) };
  const second = {
    cursor: "second",
    document: { ...first.document, projectInstanceId: "project-2" },
  };
  let unmounted = false;
  try {
    startProjectLifecycle("project-1");
    useProjectIOStore.setState({ projectInstanceId: "project-1" });
    act(() => root.render(<Probe />));
    act(() => {
      startProjectLifecycle("project-2");
      useProjectIOStore.setState({ projectInstanceId: "project-2" });
    });
    await act(async () => pending[1](second));
    await act(async () => pending[0](first));
    expect(current!.document).toBe(second.document);
    act(() => current!.setExpanded("project.events", false));
    expect(backend).toHaveBeenCalledTimes(2);

    act(() => useResourceStore.getState().setResources([]));
    expect(current!.document).toBe(second.document);
    expect(backend).toHaveBeenLastCalledWith(
      "project",
      { projectInstanceId: "project-2" },
      "en-US",
      second,
    );
    act(() => useResourceStore.getState().setResources([]));
    act(() => useResourceStore.getState().setResources([]));
    expect(backend).toHaveBeenCalledTimes(3);
    const intermediate = {
      cursor: "intermediate",
      document: { ...second.document, publicationRevision: 2 },
    };
    await act(async () => pending[2](intermediate));
    expect(current!.document).toBe(second.document);
    expect(backend).toHaveBeenCalledTimes(4);
    expect(backend).toHaveBeenLastCalledWith(
      "project",
      { projectInstanceId: "project-2" },
      "en-US",
      intermediate,
    );
    const latest = { cursor: "latest", document: { ...second.document, publicationRevision: 3 } };
    await act(async () => pending[3](latest));
    expect(current!.document).toBe(latest.document);
    act(() => current!.refresh());
    act(() => root.unmount());
    unmounted = true;
    await act(async () => pending[4](second));
    expect(current!.document).toBe(latest.document);
    expect(backend).toHaveBeenCalledTimes(5);
  } finally {
    if (!unmounted) act(() => root.unmount());
    clearProjectLifecycle();
    useProjectIOStore.setState({ projectInstanceId: null });
  }
});
