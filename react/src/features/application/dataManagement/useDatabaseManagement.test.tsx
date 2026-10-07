// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useEditorStore } from "@/features/core/editor";
import { useResourceStore, resourceKey } from "@/features/core/resource";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { ProjectService } from "@/services/project/projectService";
import { projectIndexSnapshotFixture } from "@/tests/helpers/activityPanelFixture";
import { DatabaseService } from "@/services/database/databaseService";
import { openPathDialog } from "@/services/platform/pathDialog";
import { uiStore } from "@/features/core/ui/UIStore";
import { useDatabaseManagement } from "./useDatabaseManagement";

vi.mock("@/services/platform/pathDialog", () => ({ openPathDialog: vi.fn() }));

const projectInstanceId = "00000000-0000-0000-0000-000000000601";
const otherDatabase = {
  id: "other",
  name: "Other",
  resourcePath: "another database resource path",
  engine: { dataset: {} },
  schemaVersion: 1,
  required: false,
};
const salesDatabase = {
  id: "sales",
  name: "Sales",
  resourcePath: "opaque database resource path",
  engine: { dataset: {} },
  schemaVersion: 1,
  required: false,
};
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function aggregate(afterName: string | null, operationId: string, createdId?: string) {
  const id = createdId ?? "sales";
  const resourcePath = createdId
    ? "opaque imported database resource path"
    : salesDatabase.resourcePath;
  const fromRevision = createdId ? 0 : 4;
  const toRevision = fromRevision + 1;
  vi.mocked(ProjectService.getProjectIndex).mockResolvedValue(
    projectIndexSnapshotFixture({
      projectInstanceId,
      projectName: "Test",
      exportTime: "",
      publicationRevision: 1,
      eventGraphs: [],
      functionGraphs: [],
      minds: [],
      docs: [],
      charts: [],
      databases: [
        { ...otherDatabase, revision: 2 },
        ...(createdId ? [{ ...salesDatabase, revision: 4 }] : []),
        ...(afterName === null
          ? []
          : [
              {
                id,
                resourcePath,
                revision: toRevision,
                engine: { dataset: {} },
                schemaVersion: 1,
                required: false,
                name: afterName,
              },
            ]),
      ],
    }),
  );
  const before = {
    id,
    engine: { dataset: {} },
    schemaVersion: 1,
    required: false,
    name: "Sales",
  };
  return {
    data: null,
    mutation: {
      operationId,
      projectInstanceId,
      publicationRevision: 1,
      moves: [],
      deltas: [
        {
          resource: { kind: "database" as const, key: resourcePath },
          fromRevision,
          toRevision,
          causedBy: operationId,
          payload: {
            kind: "database" as const,
            patch: {
              before: createdId ? null : before,
              after: afterName === null ? null : { ...before, name: afterName },
            },
          },
        },
      ],
      projectionReplacements: [],
      projectionStatus: { status: "complete" as const, expectedGraphPaths: [] },
    },
  };
}

describe("useDatabaseManagement revision authority", () => {
  let root: Root;
  let host: HTMLDivElement;
  let actions: ReturnType<typeof useDatabaseManagement>;

  beforeEach(() => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    vi.spyOn(ProjectService, "getProjectIndex");
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useEditorStore.getState().clearDetailFocus();
    useResourceStore.getState().clear();
    useResourceStore.setState({
      databases: {
        other: otherDatabase,
        sales: salesDatabase,
      },
      resources: Object.fromEntries(
        (
          [
            ["sales", 4],
            ["other", 2],
          ] as const
        ).map(([id, revision]) => {
          const uri = resourceKey({ kind: "database", id });
          return [
            uri,
            {
              id,
              kind: "database",
              name: id === "sales" ? "Sales" : "Other",
              revision,
              uri,
              exists: true,
              loaded: true,
              hasDirtyDocument: false,
              hasStaleDocument: false,
              hasConflictDocument: false,
            },
          ];
        }),
      ),
    });
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    function Harness() {
      actions = useDatabaseManagement();
      return null;
    }
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    for (const modal of uiStore.getState().modals) uiStore.closeModal(modal.id);
    expect(uiStore.getState().progress).toBeNull();
  });

  it("keeps a canceled import open and closes only that dialog after a sheet finishes importing", async () => {
    vi.mocked(openPathDialog)
      .mockResolvedValueOnce({ ok: true, value: null })
      .mockResolvedValueOnce({ ok: true, value: "C:/sales.xlsx" });
    vi.spyOn(DatabaseService, "listExcelSheets").mockResolvedValue(["Sales", "Forecast"]);
    let completeImport!: () => void;
    vi.spyOn(DatabaseService, "loadDatabase").mockImplementation(
      (_project, operation) =>
        new Promise((resolve) => {
          completeImport = () =>
            resolve({
              ...aggregate("Imported sales", operation, "imported-sales"),
              data: {
                id: "imported-sales",
                name: "Imported sales",
                rowCount: 1,
                columnCount: 1,
                columns: [
                  {
                    name: "value",
                    type: "Int64",
                    physical: "Int64",
                    semantic: null,
                    supportedSemanticTypes: [
                      "Numeric",
                      "Categorical",
                      "Ordinal",
                      "Binary",
                      "Identifier",
                    ] as const,
                  },
                ],
              },
            });
        }),
    );
    actions.triggerImportData();
    const importDialog = uiStore.getState().modals[0];
    if (importDialog.type !== "import") throw new Error("missing import dialog");

    await importDialog.options.onSelect("xlsx");
    expect(uiStore.getState().modals).toEqual([importDialog]);
    expect(DatabaseService.listExcelSheets).not.toHaveBeenCalled();
    expect(DatabaseService.loadDatabase).not.toHaveBeenCalled();

    await importDialog.options.onSelect("xlsx");
    const sheetDialog = uiStore.getState().modals[1];
    if (sheetDialog.type !== "excelSheetSelect") throw new Error("missing sheet dialog");
    sheetDialog.options.onSelect("Sales");
    uiStore.closeModal(sheetDialog.id);
    await vi.waitFor(() => expect(DatabaseService.loadDatabase).toHaveBeenCalledOnce());
    expect(uiStore.getState().modals).toEqual([importDialog]);

    const noticeDismissed = uiStore.alert({
      title: "Notice",
      message: "Another dialog",
      closeText: "Close",
      type: "info",
    });
    const notice = uiStore.getState().modals[1];
    completeImport();
    await vi.waitFor(() => expect(uiStore.getState().modals).toEqual([notice]));

    expect(DatabaseService.loadDatabase).toHaveBeenCalledWith(
      projectInstanceId,
      expect.any(String),
      { excel: { path: "C:/sales.xlsx", sheet: "Sales" } },
    );
    expect(useResourceStore.getState().databases["imported-sales"]).toMatchObject({
      name: "Imported sales",
      rowCount: 1,
      columns: [
        {
          name: "value",
          type: "Int64",
          physical: "Int64",
          semantic: null,
          supportedSemanticTypes: [
            "Numeric",
            "Categorical",
            "Ordinal",
            "Binary",
            "Identifier",
          ] as const,
        },
      ],
    });
    uiStore.closeModal(notice.id);
    await noticeDismissed;
  });

  it("passes exact database revision and submits the canonical aggregate mutation", async () => {
    vi.spyOn(DatabaseService, "renameDatabase").mockImplementation(async (_project, operation) =>
      aggregate("Renamed", operation),
    );

    await act(async () => actions.renameDataFrame("sales", "Renamed"));

    expect(DatabaseService.renameDatabase).toHaveBeenCalledWith(
      projectInstanceId,
      expect.any(String),
      4,
      "sales",
      "Renamed",
    );
    expect(
      useResourceStore.getState().resources[resourceKey({ kind: "database", id: "sales" })]
        .revision,
    ).toBe(5);
    expect(useResourceStore.getState().databases.sales?.name).toBe("Renamed");
  });

  it("does not perform an independent delete outside canonical publication application", async () => {
    const progress = vi.spyOn(uiStore, "startProgress");
    vi.spyOn(DatabaseService, "deleteDatabase").mockImplementation(async (_project, operation) =>
      aggregate(null, operation),
    );

    await act(async () => actions.deleteDataFrame("sales"));

    expect(DatabaseService.deleteDatabase).toHaveBeenCalledWith(
      projectInstanceId,
      expect.any(String),
      4,
      "sales",
    );
    expect(useResourceStore.getState().databases.sales).toBeUndefined();
    expect(
      useResourceStore.getState().resources[resourceKey({ kind: "database", id: "sales" })],
    ).toBeUndefined();
    expect(progress).not.toHaveBeenCalled();
  });

  it("preserves a different detail selection made while a database deletion is pending", async () => {
    let complete!: () => void;
    vi.spyOn(DatabaseService, "deleteDatabase").mockImplementation(
      (_project, operation) =>
        new Promise((resolve) => {
          complete = () => resolve(aggregate(null, operation));
        }),
    );
    act(() => useEditorStore.getState().setDetailFocus({ kind: "data", id: "sales" }));
    const deleting = actions.deleteDataFrame("sales");
    act(() => useEditorStore.getState().setDetailFocus({ kind: "data", id: "other" }));
    await act(async () => {
      complete();
      await deleting;
    });
    expect(useEditorStore.getState().detailFocus).toEqual({ kind: "data", id: "other" });
  });
});
