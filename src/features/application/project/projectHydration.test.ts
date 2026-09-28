import { projectIndexSnapshotFixture } from "@/tests/helpers/activityPanelFixture";
import { afterEach, expect, it, vi } from "vitest";
import { buildFileResourceMeta, useResourceStore } from "@/features/core/resource";
import { ProjectService } from "@/services/project/projectService";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { refreshProjectResourceIndex } from "./projectHydration";

afterEach(() => {
  projectPublicationCoordinator.cancelProject();
  useResourceStore.getState().clear();
  vi.restoreAllMocks();
});

it("never publishes an index older than a committed resource revision", async () => {
  projectPublicationCoordinator.startProject("project-a", 2);
  useResourceStore.getState().setSnapshot({ resources: [], publicationRevision: 2 });
  const graph = buildFileResourceMeta("event_graph", "events/Deleted.yssbi-event", "Deleted");
  const index = {
    projectInstanceId: "project-a",
    projectName: "Project",
    exportTime: "",
    publicationRevision: 2,
    eventGraphs: [],
    functionGraphs: [],
    minds: [],
    docs: [],
    charts: [],
    databases: [],
  };
  const query = vi
    .spyOn(ProjectService, "getProjectIndex")
    .mockResolvedValueOnce(
      projectIndexSnapshotFixture({
        ...index,
        publicationRevision: 1,
        eventGraphs: [{ path: graph.id, name: graph.name, type: "event_graph", revision: 0 }],
        functionGraphs: [],
      }),
    )
    .mockResolvedValueOnce(projectIndexSnapshotFixture(index));
  const published: boolean[] = [];
  const unsubscribe = useResourceStore.subscribe((state) =>
    published.push(Boolean(state.resources[graph.uri])),
  );
  try {
    expect(await refreshProjectResourceIndex()).toBe(true);
    expect(query).toHaveBeenCalledTimes(2);
    expect(published).toEqual([false]);
    expect(useResourceStore.getState().indexRevision).toBe(2);
  } finally {
    unsubscribe();
  }
});
