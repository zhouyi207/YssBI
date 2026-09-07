import {
  loadCurrentProject,
  refreshProjectResourceIndex,
} from "@/features/application/project/projectHydration";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ProjectService } from "@/services/project/projectService";
import { normalizeIpcError } from "@/services/ipc";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { LoadStatus } from "@/shared/types/ui/common";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";

const loggerMocks = vi.hoisted(() => ({
  error: vi.fn(),
  info: vi.fn(),
}));

const loadGraphProjection = vi.hoisted(() => vi.fn<() => Promise<boolean>>());
const removeProjectScopedWorkbenchPanels = vi.hoisted(() => vi.fn(async () => undefined));

vi.mock("@/features/application/observability/appLogger", () => ({
  logger: {
    sys: loggerMocks,
  },
}));

vi.mock("@/services/project/projectService", () => ({
  ProjectService: {
    getProjectPath: vi.fn(),
    getDatabases: vi.fn(),
    getProjectIndex: vi.fn(),
  },
}));

vi.mock("@/features/application/project/projectWorkbenchLifecycle", () => ({
  removeProjectScopedWorkbenchPanels,
}));

vi.mock(
  "@/features/application/graphProjection/graphProjectionLifecycle",
  async (importOriginal) => ({
    ...(await importOriginal<
      typeof import("@/features/application/graphProjection/graphProjectionLifecycle")
    >()),
    beginGraphLoadLifecycle: () => 1,
    loadGraphProjection,
  }),
);

const projectInstanceId = "project-error-state-test";

describe("projectIOStore error references", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    startProjectLifecycle(projectInstanceId);
    useProjectIOStore.setState({
      status: LoadStatus.Idle,
      error: null,
      graphLoadStatus: {},
      currentPath: null,
      projectInstanceId,
    });
    vi.mocked(ProjectService.getProjectPath).mockResolvedValue(null);
    vi.mocked(ProjectService.getDatabases).mockResolvedValue({
      databases: {},
    });
  });

  afterEach(() => {
    clearProjectLifecycle();
  });

  it("does not clean project panels during initial null-to-project hydration", async () => {
    useProjectIOStore.setState({ projectInstanceId: null });
    vi.mocked(ProjectService.getProjectIndex).mockResolvedValue({
      projectInstanceId,
      publicationRevision: 0,
      projectName: "Initial project",
      graphs: [],

      charts: [],
      databases: [],
      exportTime: "",
    });

    await expect(loadCurrentProject()).resolves.not.toBeNull();

    expect(removeProjectScopedWorkbenchPanels).not.toHaveBeenCalled();
    expect(useProjectIOStore.getState()).toMatchObject({
      projectInstanceId,
      status: LoadStatus.Ready,
    });
  });

  it("maps project parser prose to an explicit contract code", async () => {
    vi.mocked(ProjectService.getProjectIndex).mockRejectedValue(
      new Error("private project index parser prose"),
    );

    await expect(loadCurrentProject()).resolves.toBeNull();

    const state = useProjectIOStore.getState();
    expect(state.status).toBe(LoadStatus.Error);
    expect(state.error).toEqual({
      code: "project_load_contract_error",
      incidentId: null,
    });
    expect(JSON.stringify(state.error)).not.toContain("private project index parser prose");
  });

  it("keeps only normalized transport code and drops transport prose", async () => {
    vi.mocked(ProjectService.getProjectPath).mockRejectedValue(
      normalizeIpcError("get_project_path", new Error("private project transport prose")),
    );

    await expect(loadCurrentProject()).resolves.toBeNull();

    expect(useProjectIOStore.getState().error).toEqual({
      code: "ipc_transport_failure",
      incidentId: null,
    });
    expect(JSON.stringify(useProjectIOStore.getState().error)).not.toContain(
      "private project transport prose",
    );
  });

  it("preserves backend code and incident ID without retaining details", async () => {
    vi.mocked(ProjectService.getProjectPath).mockRejectedValue(
      normalizeIpcError("get_project_path", {
        code: "project_io_failed",
        details: { debug: "private project backend detail" },
        incidentId: "incident-project-load-42",
      }),
    );

    await expect(loadCurrentProject()).resolves.toBeNull();

    expect(useProjectIOStore.getState().error).toEqual({
      code: "project_io_failed",
      incidentId: "incident-project-load-42",
    });
    expect(JSON.stringify(useProjectIOStore.getState().error)).not.toContain(
      "private project backend detail",
    );
  });

  it("maps resource-index parser prose to its stable contract code", async () => {
    vi.mocked(ProjectService.getProjectIndex).mockRejectedValue(
      new Error("private resource index parser prose"),
    );

    await expect(refreshProjectResourceIndex()).resolves.toBe(false);

    expect(useProjectIOStore.getState().error).toEqual({
      code: "project_resource_index_contract_error",
      incidentId: null,
    });
    expect(JSON.stringify(useProjectIOStore.getState().error)).not.toContain(
      "private resource index parser prose",
    );
  });

  it("maps graph projection rejections to a stable contract code", async () => {
    loadGraphProjection.mockRejectedValue(new Error("private graph projection prose"));

    await expect(
      useProjectIOStore.getState().loadGraph("events/ErrorStateMigration.yssbi-event"),
    ).resolves.toBe(false);

    expect(useProjectIOStore.getState().error).toEqual({
      code: "graph_projection_contract_error",
      incidentId: null,
    });
    expect(JSON.stringify(useProjectIOStore.getState().error)).not.toContain(
      "private graph projection prose",
    );
  });
});
