// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { useActivityPanelDocument } from "./useActivityPanelDocument";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { ProjectService } from "@/services/project/projectService";
import * as activityService from "@/services/workbench/activityPanelService";
import {
  projectIndexSnapshotFixture,
  activityPanelFixture,
} from "@/tests/helpers/activityPanelFixture";
import type { ProjectIndexSnapshot } from "@/shared/types/domain/project";

vi.mock("@/features/application/graphProjection/projectionLocale", () => ({
  currentProjectionLocale: () => "en-US",
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ i18n: { language: "en-US" }, t: (key: string) => key }),
}));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

it("shares project sync, retains the visible document and ignores resource-field writes and old-project replies", async () => {
  const pending: Array<(snapshot: ProjectIndexSnapshot) => void> = [];
  const query = vi
    .spyOn(ProjectService, "getProjectIndex")
    .mockImplementation(
      () => new Promise<ProjectIndexSnapshot>((resolve) => pending.push(resolve)),
    );
  const standalone = vi.spyOn(activityService, "getActivityPanelDocument");
  const response = (projectInstanceId: string, publicationRevision = 0) =>
    projectIndexSnapshotFixture({
      projectInstanceId,
      publicationRevision,
      projectName: "Project",
      exportTime: "",
      graphs: [],
      charts: [],
      databases: [],
    });
  useResourceStore.getState().clear();
  useSidebarStore.setState({ panels: {}, expandedCategories: {} });
  let current: ReturnType<typeof useActivityPanelDocument> | undefined;
  function Probe() {
    current = useActivityPanelDocument("nodes");
    return null;
  }
  const root = createRoot(document.createElement("div"));
  try {
    projectPublicationCoordinator.startProject("project-1", 0);
    useProjectIOStore.setState({ projectInstanceId: "project-1" });
    await act(async () => root.render(<Probe />));
    expect(query).toHaveBeenCalledOnce();
    await act(async () => {
      projectPublicationCoordinator.startProject("project-2", 0);
      useProjectIOStore.setState({ projectInstanceId: "project-2" });
    });
    expect(query).toHaveBeenCalledTimes(2);
    const second = response("project-2");
    await act(async () => pending[1](second));
    await act(async () => pending[0](response("project-1")));
    expect(current!.document).toBe(second.activityPanels.nodes.document);
    act(() => current!.setExpanded("category", false));
    act(() => useResourceStore.getState().setSnapshot({ resources: [] }));
    expect(query).toHaveBeenCalledTimes(2);

    await act(async () => current!.refresh());
    expect(current!.document).toBe(second.activityPanels.nodes.document);
    expect(query).toHaveBeenCalledTimes(3);
    expect(query.mock.calls[2][2]?.nodes).toBe(second.activityPanels.nodes);
    await act(async () => {
      current!.refresh();
      current!.refresh();
    });
    expect(query).toHaveBeenCalledTimes(3);
    const intermediate = response("project-2", 1);
    await act(async () => pending[2](intermediate));
    expect(query).toHaveBeenCalledTimes(4);
    expect(current!.document).toBe(intermediate.activityPanels.nodes.document);
    const latest = response("project-2", 2);
    await act(async () => pending[3](latest));
    expect(current!.document).toBe(latest.activityPanels.nodes.document);
    expect(current!.loading).toBe(false);
    expect(standalone).not.toHaveBeenCalled();
    query
      .mockRejectedValueOnce({ code: "panel_sync_failed", incidentId: "panel-42" })
      .mockRejectedValueOnce({ code: "panel_sync_failed", incidentId: "panel-42" });
    await act(async () => current!.refresh());
    expect(current!.document).toBe(latest.activityPanels.nodes.document);
    expect(current!.error).toContain("panel_sync_failed");
    expect(current!.error).toContain("panel-42");
  } finally {
    act(() => root.unmount());
    projectPublicationCoordinator.cancelProject();
    useProjectIOStore.setState({ projectInstanceId: null });
    vi.restoreAllMocks();
  }
});

it("reuses global panel documents across mounts and preserves them when refresh fails", async () => {
  useSidebarStore.setState({ panels: {} });
  const snapshot = { cursor: "commands-1", document: activityPanelFixture("commands", []) };
  const backend = vi
    .spyOn(activityService, "getActivityPanelDocument")
    .mockResolvedValueOnce(snapshot);
  const indexQuery = vi.spyOn(ProjectService, "getProjectIndex");
  let current: ReturnType<typeof useActivityPanelDocument> | undefined;
  function Probe() {
    current = useActivityPanelDocument("commands");
    return null;
  }
  let root = createRoot(document.createElement("div"));
  try {
    await act(async () => root.render(<Probe />));
    expect(current!.document).toBe(snapshot.document);
    act(() => root.unmount());
    root = createRoot(document.createElement("div"));
    await act(async () => root.render(<Probe />));
    expect(backend).toHaveBeenCalledOnce();
    backend.mockRejectedValueOnce({
      code: "activity_query_failed",
      incidentId: "incident-42",
      details: { private: "not for UI" },
    });
    await act(async () => current!.refresh());
    expect(current!.document).toBe(snapshot.document);
    expect(current!.error).toContain("activity_query_failed");
    expect(current!.error).toContain("incident-42");
    expect(current!.error).not.toContain("not for UI");
    expect(current!.loading).toBe(false);
    expect(indexQuery).not.toHaveBeenCalled();
  } finally {
    act(() => root.unmount());
    vi.restoreAllMocks();
  }
});
