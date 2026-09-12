import * as projectHydration from "@/features/application/project/projectHydration";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  useDatabaseStore,
  useGraphProjectionStore,
  useGraphMetaStore,
} from "@/features/core/dataStore";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useResourceStore } from "@/features/core/resource";
import { DatabaseService } from "@/services/database/databaseService";
import { GraphService } from "@/services/graph/graphService";
import { ChartService } from "@/services/chart/chartService";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { deleteResource, renameResource } from "./resourceActions";

vi.mock("@/features/application/editorMutation/projectPublicationCoordinator", () => ({
  projectPublicationCoordinator: {
    submit: vi.fn(async () => ({ status: "applied", affectedGraphPaths: new Set() })),
    capturePublicationRevision: vi.fn(() => 0),
  },
}));

function databaseResult(afterName: string | null, operationId: string) {
  const before = {
    id: "sales",
    engine: { dataset: {} },
    schemaVersion: 1,
    required: false,
    name: "Sales",
  };
  return {
    data: null,
    mutation: {
      operationId,
      projectInstanceId: "project-instance-current",
      publicationRevision: 1,
      moves: [],
      deltas: [
        {
          resource: { kind: "database" as const, key: "opaque database resource path" },
          fromRevision: 4,
          toRevision: 5,
          causedBy: operationId,
          payload: {
            kind: "database" as const,
            patch: { before, after: afterName === null ? null : { ...before, name: afterName } },
          },
        },
      ],
      projectionReplacements: [],
      projectionStatus: { status: "complete" as const, expectedGraphPaths: [] },
    },
  };
}

function deleteResult(projectInstanceId: string) {
  return {
    operationId: "00000000-0000-0000-0000-000000000123",
    projectInstanceId,
    publicationRevision: 1,
    moves: [],
    deltas: [
      {
        resource: { kind: "graph" as const, key: "events/Old.yssbi-event" },
        fromRevision: 0,
        toRevision: 1,
        causedBy: "00000000-0000-0000-0000-000000000123",
        payload: {
          kind: "resource_lifecycle" as const,
          patch: {
            before: {
              path: "events/Old.yssbi-event",
              kind: "event" as const,
              name: "Old",
              revision: 0,
            },
            after: null,
          },
        },
      },
    ],
    projectionReplacements: [],
    projectionStatus: {
      status: "complete" as const,
      expectedGraphPaths: [],
    },
  };
}

function chartRenameResult(projectInstanceId: string, publicationRevision = 1) {
  return {
    operationId: "00000000-0000-0000-0000-000000000124",
    projectInstanceId,
    publicationRevision,
    moves: [
      {
        from: "charts/Report.yssbi-chart",
        to: "charts/Renamed Report.yssbi-chart",
        kind: "chart" as const,
        name: "Renamed Report",
      },
    ],
    deltas: [
      {
        resource: { kind: "chart" as const, key: "charts/Renamed Report.yssbi-chart" },
        fromRevision: 4,
        toRevision: 5,
        causedBy: "00000000-0000-0000-0000-000000000124",
        payload: {
          kind: "resource_move" as const,
          patch: {
            from: "charts/Report.yssbi-chart",
            to: "charts/Renamed Report.yssbi-chart",
          },
        },
      },
    ],
    projectionReplacements: [],
    projectionStatus: {
      status: "complete" as const,
      expectedGraphPaths: [],
    },
  };
}

function renameResult(projectInstanceId: string, publicationRevision = 1) {
  return {
    operationId: "00000000-0000-0000-0000-000000000123",
    projectInstanceId,
    publicationRevision,
    moves: [
      {
        from: "events/Old.yssbi-event",
        to: "events/New.yssbi-event",
        kind: "event" as const,
        name: "New",
      },
    ],
    deltas: [
      {
        resource: { kind: "graph" as const, key: "events/New.yssbi-event" },
        fromRevision: 0,
        toRevision: 1,
        causedBy: "00000000-0000-0000-0000-000000000123",
        payload: {
          kind: "resource_move" as const,
          patch: {
            from: "events/Old.yssbi-event",
            to: "events/New.yssbi-event",
          },
        },
      },
    ],
    projectionReplacements: [],
    projectionStatus: {
      status: "incomplete" as const,
      invalidatedGraphPaths: ["events/New.yssbi-event"],
    },
  };
}

describe("renameResource project ownership", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    useResourceStore.getState().clear();
    useGraphProjectionStore.setState({ graphEntities: {} });
    useResourceStore.getState().setSnapshot({
      resources: [
        {
          id: "events/Old.yssbi-event",
          kind: "event",
          name: "Old",
          uri: "yssbi://event/events/Old.yssbi-event",
          revision: 0,
          exists: true,
          loaded: false,
          hasDirtyDocument: false,
          hasStaleDocument: false,
          hasConflictDocument: false,
        },
        {
          id: "charts/Report.yssbi-chart",
          kind: "chart",
          name: "Report",
          uri: "yssbi://chart/charts/Report.yssbi-chart",
          revision: 4,
          exists: true,
          loaded: true,
          hasDirtyDocument: false,
          hasStaleDocument: false,
          hasConflictDocument: false,
        },
      ],
      graphOrder: ["events/Old.yssbi-event"],
    });
    useGraphMetaStore.getState().clear();
    useDatabaseStore.setState({
      databases: {
        sales: { id: "sales", name: "Sales", resourcePath: "opaque database resource path" },
      },
      revisions: { sales: 4 },
    });
    vi.spyOn(projectHydration, "refreshProjectResourceIndex").mockResolvedValue(true);
    useProjectIOStore.setState({
      projectInstanceId: "project-instance-current",
    });
    startProjectLifecycle("project-instance-current");
  });

  it("routes database rename and delete through exact revisioned canonical receipts", async () => {
    let renamed!: ReturnType<typeof databaseResult>;
    let deleted!: ReturnType<typeof databaseResult>;
    vi.spyOn(DatabaseService, "renameDatabase").mockImplementation(
      async (_project, operation) => (renamed = databaseResult("Renamed", operation)),
    );
    vi.spyOn(DatabaseService, "deleteDatabase").mockImplementation(
      async (_project, operation) => (deleted = databaseResult(null, operation)),
    );

    await renameResource({ id: "sales", kind: "database" }, "Renamed");
    await deleteResource({ id: "sales", kind: "database" });

    expect(DatabaseService.renameDatabase).toHaveBeenCalledWith(
      "project-instance-current",
      expect.any(String),
      4,
      "sales",
      "Renamed",
    );
    expect(DatabaseService.deleteDatabase).toHaveBeenCalledWith(
      "project-instance-current",
      expect.any(String),
      4,
      "sales",
    );
    expect(projectPublicationCoordinator.submit).toHaveBeenNthCalledWith(1, {
      result: renamed.mutation,
    });
    expect(projectPublicationCoordinator.submit).toHaveBeenNthCalledWith(2, {
      result: deleted.mutation,
    });
  });

  it("submits the authoritative delete publication", async () => {
    const committed = deleteResult("project-instance-current");
    vi.spyOn(GraphService, "removeGraph").mockResolvedValue(committed);

    await deleteResource({ id: "events/Old.yssbi-event", kind: "event" });

    expect(GraphService.removeGraph).toHaveBeenCalledWith(
      "project-instance-current",
      "events/Old.yssbi-event",
      0,
      expect.any(String),
    );
    expect(projectPublicationCoordinator.submit).toHaveBeenCalledWith({ result: committed });
    expect(projectHydration.refreshProjectResourceIndex).not.toHaveBeenCalled();
  });

  it("rejects stale chart project and lifecycle ownership before publication", async () => {
    vi.spyOn(ChartService, "renameChart").mockResolvedValueOnce(
      chartRenameResult("project-instance-stale"),
    );

    await expect(
      renameResource({ id: "charts/Report.yssbi-chart", kind: "chart" }, "Renamed Report"),
    ).rejects.toThrow("stale project lifecycle");
    expect(projectPublicationCoordinator.submit).not.toHaveBeenCalled();

    let resolveFirst!: (result: ReturnType<typeof chartRenameResult>) => void;
    let resolveSecond!: (result: ReturnType<typeof chartRenameResult>) => void;
    vi.mocked(ChartService.renameChart)
      .mockReset()
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
      )
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveSecond = resolve;
        }),
      );

    const first = renameResource(
      { id: "charts/Report.yssbi-chart", kind: "chart" },
      "Renamed Report",
    );
    await vi.waitFor(() => expect(ChartService.renameChart).toHaveBeenCalledTimes(1));
    const second = renameResource(
      { id: "charts/Report.yssbi-chart", kind: "chart" },
      "Renamed Report",
    );
    await vi.waitFor(() => expect(ChartService.renameChart).toHaveBeenCalledTimes(2));
    resolveFirst(chartRenameResult("project-instance-current", 1));
    await expect(first).rejects.toMatchObject({ code: "stale_resource_lifecycle" });
    resolveSecond(chartRenameResult("project-instance-current", 2));
    await expect(second).resolves.toBeUndefined();
  });

  it("rejects a stale rename receipt before coordinator submission", async () => {
    vi.spyOn(GraphService, "renameGraphResource").mockResolvedValue(
      renameResult("project-instance-stale"),
    );

    await expect(
      renameResource({ id: "events/Old.yssbi-event", kind: "event" }, "New"),
    ).rejects.toThrow("stale project lifecycle");

    expect(projectPublicationCoordinator.submit).not.toHaveBeenCalled();
  });

  it("rejects a matching receipt when project ownership changes in flight", async () => {
    let resolveRename!: (
      value: Awaited<ReturnType<typeof GraphService.renameGraphResource>>,
    ) => void;
    vi.spyOn(GraphService, "renameGraphResource").mockReturnValue(
      new Promise((resolve) => {
        resolveRename = resolve;
      }),
    );

    const pending = renameResource({ id: "events/Old.yssbi-event", kind: "event" }, "New");
    useProjectIOStore.setState({ projectInstanceId: "project-instance-replacement" });
    startProjectLifecycle("project-instance-replacement");
    resolveRename(renameResult("project-instance-current"));

    await expect(pending).rejects.toMatchObject({ code: "stale_project_lifecycle" });
    expect(projectPublicationCoordinator.submit).not.toHaveBeenCalled();
  });
});
