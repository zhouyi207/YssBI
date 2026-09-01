import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { DEFAULT_WORKSHEET_NAME } from "@/shared/constants/defaultResourceNames";
import { useWorksheetStore } from "@/features/core/worksheet/worksheetStore";
import { commitFileFirstResourceIndex } from "@/features/application/resource/resourceActions";
import { WorksheetService } from "@/services/worksheet/worksheetService";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { captureProjectCommandContext } from "@/features/application/projectCommandContext";

import { revealWorkbenchView } from "@/features/application/layout/workbenchLayoutActions";
import { PROJECT_TREE_CATEGORY_IDS, useSidebarStore } from "@/features/core/sidebar";
import { isEditorOpenRejectionHandled, openEditorPanel } from "./openEditorPanel";
import type { WorksheetDocument } from "@/shared/types/domain/worksheet";
import { showBlockingIpcError } from "./blockingErrorDialog";

interface StagedWorksheetDocument {
  worksheetPath: string;
  staged: WorksheetDocument;
  previous: WorksheetDocument | undefined;
}

function stageWorksheetDocument(
  worksheetPath: string,
  staged: WorksheetDocument,
): StagedWorksheetDocument {
  const previous = useWorksheetStore.getState().documents[worksheetPath];
  useWorksheetStore.setState((state) => ({
    documents: { ...state.documents, [worksheetPath]: staged },
  }));
  return { worksheetPath, staged, previous };
}

function rollbackStagedWorksheetDocument(stage: StagedWorksheetDocument | undefined): void {
  if (!stage) return;
  useWorksheetStore.setState((state) => {
    if (state.documents[stage.worksheetPath] !== stage.staged) return {};
    const documents = { ...state.documents };
    if (stage.previous) documents[stage.worksheetPath] = stage.previous;
    else delete documents[stage.worksheetPath];
    return { documents };
  });
}

function createdWorksheetState(
  result: import("@/shared/types/domain/editorMutation").ResourceMutationResultDto,
  operationId: string,
) {
  const lifecycle = result.deltas.find(
    (delta) =>
      delta.resource.kind === "worksheet" &&
      delta.causedBy === operationId &&
      delta.payload.kind === "resource_lifecycle" &&
      delta.payload.patch.before === null &&
      delta.payload.patch.after?.kind === "worksheet",
  );
  return lifecycle?.payload.kind === "resource_lifecycle" ? lifecycle.payload.patch.after : null;
}

export function useWorksheetManagement(
  openWorksheet: (worksheetPath: string, name: string) => Promise<void>,
) {
  const { t } = useTranslation();

  const addWorksheet = useCallback(
    async (databaseId?: string) => {
      let context: ReturnType<typeof captureProjectCommandContext> | undefined;
      let stagedDocument: StagedWorksheetDocument | undefined;
      try {
        context = captureProjectCommandContext();
        const created = await WorksheetService.createWorksheet(
          context.projectInstanceId,
          context.operationId,
          DEFAULT_WORKSHEET_NAME,
          databaseId,
        );
        if (!context.isCurrent()) return;
        const createdState = createdWorksheetState(created, context.operationId);
        if (!createdState) throw new Error("worksheet create result has no lifecycle insert");
        const createdDocument = await WorksheetService.loadWorksheet(
          context.projectInstanceId,
          createdState.path,
        );
        if (!context.isCurrent()) return;
        stagedDocument = stageWorksheetDocument(createdState.path, createdDocument);
        await projectPublicationCoordinator.submit({ result: created });
        stagedDocument = undefined;
        if (!context.isCurrent()) return;

        await commitFileFirstResourceIndex();
        if (!context.isCurrent()) return;
        await openWorksheet(createdState.path, createdState.name);
        if (!context.isCurrent()) return;
      } catch (error) {
        if (context && !context.isCurrent()) return;
        rollbackStagedWorksheetDocument(stagedDocument);
        if (isEditorOpenRejectionHandled(error)) return;
        showBlockingIpcError(error, "create_worksheet", (code) =>
          t("notifications.worksheet.createFailed", { error: code }),
        );
      }
    },
    [openWorksheet, t],
  );

  const duplicateWorksheet = useCallback(
    async (worksheetPath: string) => {
      let context: ReturnType<typeof captureProjectCommandContext> | undefined;
      let stagedDocument: StagedWorksheetDocument | undefined;
      try {
        context = captureProjectCommandContext();
        const indexEntry = useWorksheetStore
          .getState()
          .index.find((worksheet) => worksheet.worksheetPath === worksheetPath);
        if (!indexEntry) throw new Error("worksheet has no authoritative index revision");
        const duplicated = await WorksheetService.duplicateWorksheet(
          context.projectInstanceId,
          context.operationId,
          worksheetPath,
          indexEntry.revision,
        );
        if (!context.isCurrent()) return;
        const duplicatedState = createdWorksheetState(duplicated, context.operationId);
        if (!duplicatedState) throw new Error("worksheet duplicate result has no lifecycle insert");
        const duplicatedDocument = await WorksheetService.loadWorksheet(
          context.projectInstanceId,
          duplicatedState.path,
        );
        if (!context.isCurrent()) return;
        stagedDocument = stageWorksheetDocument(duplicatedState.path, duplicatedDocument);
        await projectPublicationCoordinator.submit({ result: duplicated });
        stagedDocument = undefined;
        if (!context.isCurrent()) return;
        await commitFileFirstResourceIndex();
        if (!context.isCurrent()) return;
        await openWorksheet(duplicatedState.path, duplicatedState.name);
      } catch (error) {
        if (context && !context.isCurrent()) return;
        rollbackStagedWorksheetDocument(stagedDocument);
        if (isEditorOpenRejectionHandled(error)) return;
        showBlockingIpcError(error, "duplicate_worksheet", (code) =>
          t("notifications.worksheet.duplicateFailed", { error: code }),
        );
      }
    },
    [openWorksheet, t],
  );

  const ensureWorksheetLoaded = useCallback(async (worksheetPath: string) => {
    const cached = useWorksheetStore.getState().documents[worksheetPath];
    if (cached) return cached;
    const context = captureProjectCommandContext();
    const document = await WorksheetService.loadWorksheet(context.projectInstanceId, worksheetPath);
    if (!context.isCurrent()) return null;
    useWorksheetStore.getState().upsertDocument(worksheetPath, document);
    return document;
  }, []);

  return { addWorksheet, duplicateWorksheet, ensureWorksheetLoaded };
}

export function useOpenWorksheet() {
  return useCallback(async (worksheetPath: string, _name: string) => {
    if (!useWorksheetStore.getState().documents[worksheetPath]) {
      const context = captureProjectCommandContext();
      try {
        const loaded = await WorksheetService.loadWorksheet(
          context.projectInstanceId,
          worksheetPath,
        );
        if (!context.isCurrent()) return;
        useWorksheetStore.getState().upsertDocument(worksheetPath, loaded);
      } catch {
        if (!context.isCurrent()) return;
        // Index-only open: WorksheetEditor retries load on mount.
      }
    }

    try {
      await openEditorPanel(
        { resourceRef: worksheetPath, resourceKind: "worksheet", pinned: true },
        {
          focusDetail: { kind: "worksheet", worksheetPath },
        },
      );
      void revealWorkbenchView("project");
      useSidebarStore
        .getState()
        .setProjectTreeCategoryExpanded(PROJECT_TREE_CATEGORY_IDS.worksheets, true);
    } catch (error) {
      if (!isEditorOpenRejectionHandled(error)) throw error;
    }
  }, []);
}
