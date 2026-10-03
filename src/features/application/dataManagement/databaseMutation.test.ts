import { beforeEach, describe, expect, it, vi } from "vitest";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { resourceKey } from "@/features/core/resource/resourceTypes";
import { executeDatabaseCreate, executeDatabaseMutation } from "./databaseMutation";

const projectInstanceId = "00000000-0000-0000-0000-000000000601";
const replacementProjectInstanceId = "00000000-0000-0000-0000-000000000602";

function aggregate(operationId: string) {
  return {
    data: "done",
    mutation: {
      operationId,
      projectInstanceId,
      publicationRevision: 1,
      moves: [],
      deltas: [],
      projectionReplacements: [],
      projectionStatus: { status: "complete" as const, expectedGraphPaths: [] },
    },
  };
}

describe("executeDatabaseMutation", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useResourceStore.getState().clear();
    useResourceStore.getState().setSnapshot({
      resources: [
        {
          id: "sales",
          kind: "database",
          name: "Sales",
          revision: 4,
          uri: resourceKey({ kind: "database", id: "sales" }),
          exists: true,
          loaded: true,
          hasDirtyDocument: false,
          hasStaleDocument: false,
          hasConflictDocument: false,
        },
      ],
    });
  });

  it("passes one revisioned lifecycle snapshot to the command and settles its receipt", async () => {
    const command = vi.fn(async (authority) => aggregate(authority.operationId));

    await expect(executeDatabaseMutation("sales", command)).resolves.toBe("done");
    expect(command).toHaveBeenCalledWith({
      projectInstanceId,
      operationId: expect.any(String),
      expectedRevision: 4,
    });
  });

  it("installs import metadata only while the receipt's created revision is still current", async () => {
    const submit = vi.spyOn(projectPublicationCoordinator, "submit");
    for (const installedRevision of [1, 2]) {
      const id = `import-${installedRevision}`;
      const resourcePath = `opaque imported resource ${id}`;
      const declaration = {
        id,
        name: id,
        engine: { dataset: {} },
        schemaVersion: 1,
        required: false,
      };
      const data = {
        id,
        name: id,
        columns: [{ name: "old", type: "Int64" }],
        rowCount: 1,
        columnCount: 1,
      };
      let published = useResourceStore.getState();
      submit.mockImplementationOnce(async () => {
        const current = useResourceStore.getState();
        const resource = current.resources[resourceKey({ kind: "database", id: "sales" })]!;
        current.setSnapshot({
          resources: [
            ...Object.values(current.resources),
            {
              ...resource,
              id,
              name: id,
              uri: resourceKey({ kind: "database", id }),
              revision: installedRevision,
            },
          ],
          databases: {
            ...current.databases,
            [id]:
              installedRevision === 1
                ? { ...declaration, resourcePath }
                : {
                    ...declaration,
                    resourcePath,
                    rowCount: 99,
                    columnCount: 1,
                    columns: [{ name: "new", type: "Utf8" }],
                  },
          },
          publicationRevision: installedRevision,
        });
        published = useResourceStore.getState();
        return { status: "recovered", affectedGraphPaths: new Set() };
      });
      await executeDatabaseCreate(async ({ operationId }) => ({
        data,
        mutation: {
          ...aggregate(operationId).mutation,
          deltas: [
            {
              resource: { kind: "database", key: resourcePath },
              fromRevision: 0,
              toRevision: 1,
              causedBy: operationId,
              payload: { kind: "database", patch: { before: null, after: declaration } },
            },
          ],
        },
      }));
      const current = useResourceStore.getState();
      expect(current.databases[id]?.rowCount).toBe(installedRevision === 1 ? 1 : 99);
      if (installedRevision === 2) expect(current).toBe(published);
    }
    expect(submit).toHaveBeenCalledTimes(2);
  });

  it("rejects lifecycle replacement inside the authority reader before command or publication effects", async () => {
    const authority = useResourceStore.getState();
    const before = {
      databases: structuredClone(authority.databases),
      resources: structuredClone(authority.resources),
    };
    vi.spyOn(useResourceStore, "getState").mockImplementationOnce(() => {
      projectPublicationCoordinator.startProject(replacementProjectInstanceId, 0);
      return authority;
    });
    const command = vi.fn();
    const submit = vi.spyOn(projectPublicationCoordinator, "submit");

    await expect(executeDatabaseMutation("sales", command)).rejects.toMatchObject({
      code: "stale_project_lifecycle",
    });

    expect(command).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
    expect(useResourceStore.getState()).toMatchObject(before);
  });

  it("rejects missing revision authority before command or publication effects", async () => {
    useResourceStore.getState().clear();
    const command = vi.fn();
    const submit = vi.spyOn(projectPublicationCoordinator, "submit");

    await expect(executeDatabaseMutation("sales", command)).rejects.toThrow(
      "Database 'sales' has no authoritative revision",
    );

    expect(command).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
  });
});
