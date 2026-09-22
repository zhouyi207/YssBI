import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { loadActivatedProject } from "@/features/application/project/projectHydration";
import { resolveActiveProjectPath } from "@/features/application/project/projectSession";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { ProjectService } from "@/services/project/projectService";
import { openPathDialog } from "@/services/platform/pathDialog";
import { saveAllDirtyGraphs } from "./saveAllDirtyGraphs";
import { cancelActiveGraphRun } from "./cancelActiveGraphRun";
import { installGraphRunEvent } from "./observeGraphRunEvent";
import { recoverGraphExecution } from "@/features/application/graphProjection/graphActivity";
import { useExecutionStore, graphHasClearableArtifacts } from "@/features/core/execution";
import { isGraphProjectionExecutable } from "@/features/core/dataStore/graphEntityAccess";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { getExecutionEventTarget, resolveExecutionGraphPath } from "./resolveExecutionGraphPath";

import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { logger } from "@/features/application/observability/appLogger";
import {
  ProjectLifecycleProtocolError,
  applyProjectLifecycleReceipt,
  cancelPendingProjectLifecycleOperation,
  claimProjectLifecycleInitiatorSettlement,
  recoverProjectLifecycleDirectFailure,
  registerPendingProjectLifecycleOperation,
  type PendingProjectLifecycleOperation,
} from "@/features/application/projectLifecycleReceipt";
import { createProjectLifecycleReceiptDependencies } from "@/features/application/projectLifecycleReceiptDependencies";
import { saveChartDocument } from "@/features/application/chart/saveChartDocument";
import { showBlockingIpcError, showBlockingMessage } from "./blockingErrorDialog";
import {
  captureActiveEditorCommandTarget,
  isEditorCommandTargetCurrent,
  type EditorCommandTarget,
} from "./editorCommandFocus";
import { saveGraph as saveCurrentGraph } from "@/features/application/graphEditing/saveGraph";
import { useGraphEditingStore } from "@/features/core/graphEditing";
import { enqueueGraphTask } from "@/features/application/graphEditing/graphEditCoordinator";
import { normalizeApplicationIpcError } from "@/features/application/errorReference";

function projectParentDirectory(metadataOrRootPath: string): string {
  const normalized = metadataOrRootPath.replace(/\\/g, "/");
  const root = normalized.replace(/\/metadata\.yssbi$/i, "");
  const index = root.lastIndexOf("/");
  return index > 0 ? root.slice(0, index) : root;
}

/**
 * Project Operations Hook
 * Handles flush, load, and execute operations
 */
export function useProjectOperations() {
  const { t } = useTranslation();

  const saveGraphAs = useCallback(async () => {
    let pending: PendingProjectLifecycleOperation | undefined;
    try {
      pending = registerPendingProjectLifecycleOperation({ kind: "saveAs" });
      const projectPath = await resolveActiveProjectPath();
      if (!pending.isCurrent()) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        return;
      }
      if (!projectPath) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        showBlockingMessage(t("notifications.project.notLoaded"));
        return;
      }
      const dirtySaved = await saveAllDirtyGraphs();
      if (!pending.isCurrent()) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        return;
      }
      if (!dirtySaved) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        return;
      }

      const currentPath = await ProjectService.getProjectPath(pending.projectInstanceId!);
      if (!pending.isCurrent()) return;
      if (!currentPath) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        showBlockingMessage(t("notifications.project.notLoaded"));
        return;
      }

      const selection = await openPathDialog({
        directory: true,
        multiple: false,
        title: "项目另存为",
        defaultPath: projectParentDirectory(currentPath) || undefined,
      });
      if (!pending.isCurrent()) return;
      if (!selection.ok) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        showBlockingMessage(
          t("notifications.project.saveAsFailed", {
            error: selection.failure.code,
          }),
        );
        return;
      }
      const destination = selection.value;
      if (!destination || Array.isArray(destination)) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        return;
      }

      const result = await ProjectService.saveProjectAs(
        pending.projectInstanceId!,
        pending.operationId,
        destination,
      );
      if (!pending.isCurrent()) return;
      const settlement = await applyProjectLifecycleReceipt(
        result,
        "direct",
        createProjectLifecycleReceiptDependencies(),
      );
      if (settlement.status === "stale" || !pending.isCurrent()) return;
      claimProjectLifecycleInitiatorSettlement(pending.operationId);
    } catch (e) {
      if (e instanceof ProjectLifecycleProtocolError && e.zeroEffects) return;
      if (pending) {
        const recovered = await recoverProjectLifecycleDirectFailure(pending.operationId);
        if (recovered && pending.isCurrent()) {
          claimProjectLifecycleInitiatorSettlement(pending.operationId);
          return;
        }
        if (!pending.isCurrent()) return;
      }
      logger.app.error(String(e), "ProjectOperations");
      showBlockingIpcError(e, (code) => t("notifications.project.saveAsFailed", { error: code }));
    }
  }, [t]);

  const saveGraph = useCallback(
    async (requestedTarget?: EditorCommandTarget) => {
      const target = requestedTarget ?? captureActiveEditorCommandTarget();
      if (!target) {
        showBlockingMessage(t("notifications.project.openResourceBeforeSaving"));
        return;
      }
      if (!isEditorCommandTargetCurrent(target)) return;

      try {
        const projectPath = await resolveActiveProjectPath();
        if (!isEditorCommandTargetCurrent(target)) return;
        if (!projectPath) {
          showBlockingMessage(t("notifications.project.notLoaded"));
          return;
        }

        if (target.resourceKind === "chart") {
          const saved = await saveChartDocument(target.resourceRef);
          if (!isEditorCommandTargetCurrent(target)) return;
          if (!saved) {
            showBlockingMessage(
              t("notifications.project.saveFailed", {
                error: "chart_save_not_committed",
              }),
            );
          }
          return;
        }

        const saved = await saveCurrentGraph(target.resourceRef, target.resourceKind);
        if (!isEditorCommandTargetCurrent(target)) return;
        if (!saved) {
          showBlockingMessage(
            t("notifications.project.saveFailed", { error: "graph_save_not_committed" }),
          );
        }
      } catch (e) {
        if (!isEditorCommandTargetCurrent(target)) return;
        logger.app.error(String(e), "ProjectOperations");
        showBlockingIpcError(e, (code) => t("notifications.project.saveFailed", { error: code }));
      }
    },
    [t],
  );

  const importGraph = useCallback(async () => {
    try {
      const selection = await openPathDialog({
        multiple: false,
        filters: [{ name: "YssBI Project", extensions: ["yssbi"] }],
      });
      if (!selection.ok) {
        showBlockingMessage(`${t("notifications.project.loadFailed")} (${selection.failure.code})`);
        return;
      }
      const path = selection.value;
      if (!path) return;
      if (Array.isArray(path)) return;

      const activation = await ProjectService.loadProjectToState(path);

      const loadReceipt = await loadActivatedProject(activation);
      if (!loadReceipt) {
        showBlockingMessage(t("notifications.project.loadFailed"));
        return;
      }
    } catch (e) {
      logger.app.error(String(e), "ProjectOperations");
      showBlockingIpcError(e, (code) => `${t("notifications.project.loadFailed")} (${code})`);
    }
  }, [t]);

  const executeGraph = useCallback(
    async (targetGraphPath?: string) => {
      const graphPath = resolveExecutionGraphPath(targetGraphPath);
      if (!graphPath) return;

      const target = getExecutionEventTarget(graphPath);
      if (!target) return;

      let project: ProjectIdentitySnapshot;
      try {
        project = captureProjectIdentity();
      } catch {
        return;
      }

      let isCurrentRun: (() => boolean) | undefined;
      try {
        const started = await enqueueGraphTask(
          graphPath,
          async () => {
            if (!isCurrentProjectIdentity(project)) return null;
            const draft = useGraphEditingStore.getState().sessions[graphPath];
            if (!draft || draft.saving) return null;
            const projection = useGraphProjectionStore.getState().graphEntities[graphPath];
            if (!isGraphProjectionExecutable(projection)) {
              showBlockingMessage(t("notifications.project.problemsBlockExecution"));
              return null;
            }
            logger.exec.info(`执行当前 Analysis Graph: ${target.name} (${graphPath})`);

            isCurrentRun = useExecutionStore.getState().submitExecution(graphPath);

            const completion = ProjectService.executeGraph({
              projectInstanceId: project.projectInstanceId,
              graphPath,
              version: draft.version,
              semanticInputHash: draft.semanticInputHash,
              demand: { type: "default" },
              onEvent: (event) => {
                if (!isCurrentProjectIdentity(project) || !isCurrentRun?.()) return;
                if (event.kind.type !== "resultInspectionRequested") installGraphRunEvent(event);
              },
            });
            // Release the edit queue after dispatch; the run owns its captured draft.
            return { completion };
          },
          null,
        );
        if (!started) return;
        await started.completion;

        if (!isCurrentProjectIdentity(project) || !isCurrentRun?.()) return;
        await recoverGraphExecution(project);
      } catch (e) {
        if (!isCurrentProjectIdentity(project) || (isCurrentRun && !isCurrentRun())) return;
        const error = normalizeApplicationIpcError(e);
        const execution = useExecutionStore.getState();
        if (
          error.details &&
          typeof error.details === "object" &&
          "executionAccepted" in error.details &&
          error.details.executionAccepted === false
        ) {
          execution.interruptExecution(graphPath);
          showBlockingIpcError(
            error,
            (code) => `${t("notifications.project.executionRejected")} (${code})`,
          );
          return;
        }
        execution.markExecutionUnknown(graphPath);
        logger.exec.error(`Execution synchronization interrupted: ${error.code}`);
        try {
          await recoverGraphExecution(project);
        } catch {
          /* Keep the explicitly unknown state. */
        }
        if (isCurrentProjectIdentity(project) && execution.getGraph(graphPath).status === "unknown")
          showBlockingIpcError(error, () => t("notifications.project.executionStateUnknown"));
      }
    },
    [t],
  );

  const cancelGraphExecution = useCallback(async (targetGraphPath?: string) => {
    const graphPath = resolveExecutionGraphPath(targetGraphPath);
    if (!graphPath) return;
    try {
      await cancelActiveGraphRun(graphPath);
    } catch (e) {
      logger.exec.error(`中断执行失败: ${formatErrorMessage(e)}`);
    }
  }, []);

  const clearGraphArtifacts = useCallback(async (targetGraphPath?: string) => {
    const graphPath = resolveExecutionGraphPath(targetGraphPath);
    if (!graphPath) return;

    const store = useExecutionStore.getState();
    const graphState = store.getGraph(graphPath);
    if (graphState.status === "running") {
      return;
    }
    if (!graphHasClearableArtifacts(graphState)) {
      return;
    }

    store.clearGraphRunProjections(graphPath);
  }, []);

  return {
    saveGraph,
    saveGraphAs,
    importGraph,
    executeGraph,
    cancelGraphExecution,
    clearGraphArtifacts,
  };
}
