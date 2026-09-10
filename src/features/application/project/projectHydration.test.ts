import { projectIndexSnapshotFixture } from "@/tests/helpers/activityPanelFixture";
import { afterEach, expect, it, vi } from "vitest";
import { buildGraphResourceMeta, useResourceStore } from "@/features/core/resource";
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
  const graph = buildGraphResourceMeta("event", "events/Deleted.yssbi-event", "Deleted");
  const index = {
    projectInstanceId: "project-a",
    projectName: "Project",
    exportTime: "",
    publicationRevision: 2,
    graphs: [],
    charts: [],
    databases: [],
  };
  const query = vi
    .spyOn(ProjectService, "getProjectIndex")
    .mockResolvedValueOnce(
      projectIndexSnapshotFixture({
        ...index,
        publicationRevision: 1,
        graphs: [{ path: graph.id, name: graph.name, type: "event", revision: 0 }],
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
