import { projectIndexSnapshotFixture } from "@/tests/helpers/activityPanelFixture";
import { afterEach, expect, it, vi } from "vitest";
import { buildFileResourceMeta, useResourceStore } from "@/features/core/resource";
import { ProjectService } from "@/services/project/projectService";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { captureProjectIdentity } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useNodeCatalogStore } from "@/features/core/nodeCatalog/nodeCatalogStore";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import { projectSnapshotFixture } from "@/tests/helpers/projectSnapshotFixtures";
import { LoadStatus } from "@/shared/types/ui/common";
import { buildProjectResourceState } from "./authoritativeProjectLoadPlan";
import { useProjectIOStore } from "./projectIOStore";
import {
  commitPreparedAuthoritativeProjectLoad,
  loadCurrentProject,
  prepareAuthoritativeProjectLoad,
  refreshProjectResourceIndex,
} from "./projectHydration";

afterEach(() => {
  projectPublicationCoordinator.cancelProject();
  useResourceStore.getState().clear();
  useProjectIOStore.setState({ projectInstanceId: null });
  vi.restoreAllMocks();
});

it("keeps the successor hydration owner when loading observers replace the project", async () => {
  const { plan } = projectSnapshotFixture(0);
  const nextIndex = { ...plan.index, projectInstanceId: "project-b", publicationRevision: 1 };
  let resolveIndex!: (snapshot: ReturnType<typeof projectIndexSnapshotFixture>) => void;
  const pendingIndex = new Promise<ReturnType<typeof projectIndexSnapshotFixture>>((resolve) => {
    resolveIndex = resolve;
  });
  const queryIndex = vi
    .spyOn(ProjectService, "getProjectIndex")
    .mockImplementation((id) =>
      id === "project-b"
        ? pendingIndex
        : Promise.resolve(projectIndexSnapshotFixture({ ...nextIndex, projectInstanceId: id })),
    );
  vi.spyOn(ProjectService, "getProjectPath").mockResolvedValue("B");
  vi.spyOn(ProjectService, "getDatabases").mockResolvedValue({ databases: {} });
  projectPublicationCoordinator.startProject("project-a", 0);
  useProjectIOStore.setState({ projectInstanceId: "project-a", status: LoadStatus.Ready });
  let successor: ReturnType<typeof loadCurrentProject> | undefined;
  const stop = useProjectIOStore.subscribe((state) => {
    if (state.status !== LoadStatus.Loading) return;
    stop();
    projectPublicationCoordinator.startProject("project-b", 0);
    useProjectIOStore.setState({ projectInstanceId: "project-b" });
    successor = loadCurrentProject();
  });
  try {
    expect(await loadCurrentProject()).toBeNull();
    expect(successor).toBeDefined();
    const repeated = loadCurrentProject();
    await Promise.resolve();
    expect.soft(queryIndex.mock.calls.filter(([id]) => id === "project-b")).toHaveLength(1);
    resolveIndex(projectIndexSnapshotFixture(nextIndex));
    const [first, second] = await Promise.all([successor, repeated]);
    expect.soft(first).toMatchObject({ projectInstanceId: "project-b", publicationRevision: 1 });
    expect.soft(second).toBe(first);
    expect(useProjectIOStore.getState().currentPath).toBe("B");
  } finally {
    stop();
    resolveIndex(projectIndexSnapshotFixture(nextIndex));
  }
});

it("loads fresh metadata against the captured index revision without retaining older row counts", async () => {
  const { plan } = projectSnapshotFixture(0);
  const index = { ...plan.index, projectInstanceId: "project-a", publicationRevision: 4 };
  index.databases = [
    {
      id: "sales",
      resourcePath: "databases/sales",
      name: "Sales",
      revision: 4,
      engine: { dataset: {} },
      schemaVersion: 1,
      required: false,
    },
  ];
  projectPublicationCoordinator.startProject("project-a", 3);
  useProjectIOStore.setState({ projectInstanceId: "project-a" });
  useResourceStore.getState().setSnapshot({
    resources: Object.values(
      buildProjectResourceState({
        ...index,
        databases: [{ ...index.databases[0], revision: 3 }],
      }).resources,
    ),
    databases: {
      sales: { id: "sales", name: "Sales", rowCount: 99, columns: [{ name: "old", type: "Utf8" }] },
    },
    publicationRevision: 3,
  });
  const before = useResourceStore.getState();
  const queryIndex = vi
    .spyOn(ProjectService, "getProjectIndex")
    .mockResolvedValue(projectIndexSnapshotFixture(index));
  const queryPath = vi
    .spyOn(ProjectService, "getProjectPath")
    .mockResolvedValue("C:/project-a/metadata.yssbi");
  const columns = [{ name: "current", type: "Float64" }];
  const queryDatabases = vi.spyOn(ProjectService, "getDatabases").mockResolvedValue({
    databases: {
      sales: {
        id: "sales",
        name: "Sales",
        engine: { dataset: {} },
        schemaVersion: 1,
        required: false,
        loadFailed: false,
        columns: columns.map((column) => ({ ...column, physical: "Float64", semantic: null })),
        columnCount: columns.length,
      },
    },
  });
  const prepared = await prepareAuthoritativeProjectLoad(captureProjectIdentity());
  expect(queryPath).toHaveBeenCalledWith("project-a");
  expect(queryDatabases).toHaveBeenCalledWith("project-a", 4);
  expect(queryIndex.mock.invocationCallOrder[0]).toBeLessThan(
    queryDatabases.mock.invocationCallOrder[0]!,
  );
  expect(prepared.storeState.databases.sales.columns).toEqual([
    { ...columns[0], physical: "Float64", semantic: null },
  ]);
  expect(prepared.storeState.databases.sales.rowCount).toBeUndefined();
  expect(useResourceStore.getState()).toBe(before);

  queryIndex.mockResolvedValue(
    projectIndexSnapshotFixture({
      ...index,
      publicationRevision: 5,
      databases: [{ ...index.databases[0], revision: 5 }],
    }),
  );
  expect(await refreshProjectResourceIndex()).toBe(true);
  const advanced = useResourceStore.getState();
  await expect(commitPreparedAuthoritativeProjectLoad(prepared)).rejects.toMatchObject({
    code: "stale_project_lifecycle",
  });
  expect(useResourceStore.getState()).toBe(advanced);
  expect(projectPublicationCoordinator.capturePublicationRevision()).toBe(5);
});

it("never publishes an index older than a committed resource revision", async () => {
  projectPublicationCoordinator.startProject("project-a", 2);
  useResourceStore.getState().setSnapshot({ resources: [], publicationRevision: 2 });
  const before = useResourceStore.getState();
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
    expect(published).toEqual([]);
    expect(useResourceStore.getState()).toBe(before);
    expect(useResourceStore.getState().indexRevision).toBe(2);
  } finally {
    unsubscribe();
  }
});

it("stops an old project commit when a publication listener installs a successor", async () => {
  for (const stage of ["catalog reset", "graph load status"] as const) {
    const { plan } = projectSnapshotFixture(0);
    const index = { ...plan.index, projectInstanceId: "project-a", publicationRevision: 1 };
    projectPublicationCoordinator.startProject("project-a", 0);
    useProjectIOStore.setState({ projectInstanceId: "project-a" });
    vi.spyOn(ProjectService, "getProjectIndex").mockResolvedValue(
      projectIndexSnapshotFixture(index),
    );
    vi.spyOn(ProjectService, "getProjectPath").mockResolvedValue("A");
    vi.spyOn(ProjectService, "getDatabases").mockResolvedValue({ databases: {} });
    const prepared = await prepareAuthoritativeProjectLoad(captureProjectIdentity());
    const resource = buildFileResourceMeta("doc", "docs/Successor.md", "Successor");
    let successorBinding: ReturnType<typeof useSidebarStore.getState>["panels"]["project"];
    let unsubscribe = () => {};
    const replaceProject = () => {
      unsubscribe();
      projectPublicationCoordinator.startProject("project-b", 9);
      useProjectIOStore.setState({ projectInstanceId: "project-b", currentPath: "B" });
      useResourceStore.getState().setSnapshot({ resources: [resource], publicationRevision: 9 });
      useSidebarStore.getState().bindPanel({
        panelId: "project",
        locale: "en",
        ...captureProjectIdentity(),
      });
      successorBinding = useSidebarStore.getState().panels.project;
    };
    unsubscribe =
      stage === "catalog reset"
        ? useNodeCatalogStore.subscribe(replaceProject)
        : useProjectIOStore.subscribe(replaceProject);
    try {
      await expect.soft(commitPreparedAuthoritativeProjectLoad(prepared)).rejects.toMatchObject({
        code: "stale_project_lifecycle",
      });
      expect.soft(captureProjectIdentity().projectInstanceId, stage).toBe("project-b");
      expect.soft(useProjectIOStore.getState().currentPath, stage).toBe("B");
      expect.soft(useProjectIOStore.getState().projectInstanceId, stage).toBe("project-b");
      expect.soft(useResourceStore.getState().resources[resource.uri], stage).toEqual(resource);
      expect.soft(useSidebarStore.getState().panels.project, stage).toBe(successorBinding);
      expect.soft(useNodeCatalogStore.getState().projectWatermarks, stage).toEqual({
        "project-b": 9,
      });
    } finally {
      unsubscribe();
    }
  }
});
