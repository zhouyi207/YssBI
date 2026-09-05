import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { loadActivatedProject } from "@/features/application/project/projectIOStore";
import { resolveActiveProjectPath } from "@/features/application/project/projectSession";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { ProjectService, isExecutionCancelledError } from "@/services/project/projectService";
import { openPathDialog } from "@/services/platform/pathDialog";
import { saveAllDirtyGraphs } from "./saveAllDirtyGraphs";
import { cancelActiveGraphRun } from "./cancelActiveGraphRun";
import {
  observeGraphRunEvent,
  observeGraphRunOutput,
  type GraphRunOutcomeState,
} from "./observeGraphRunEvent";
import { openInspectableResult } from "@/features/application/execution/openInspectableResult";
import { resultRef } from "@/features/application/results";
import { useExecutionStore, graphHasClearableArtifacts } from "@/features/core/execution";
import { isGraphProjectionExecutable } from "@/features/core/dataStore/graphEntityAccess";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { getExecutionEventTarget, resolveExecutionGraphPath } from "./resolveExecutionGraphPath";

import type { RecordedEvent } from "@/features/core/execution/executionTypes";
import { ensureGraphExecutionTerminal } from "@/features/core/execution/executionRecording";
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
import { saveGraphDraft } from "@/features/application/graphDraft/saveGraphDraft";
import { compileGraphDraft } from "@/features/application/graphDraft/compileGraphDraft";
import { useGraphDraftStore } from "@/features/core/graphDraft";

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
      showBlockingIpcError(e, "save_project_as", (code) =>
        t("notifications.project.saveAsFailed", { error: code }),
      );
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

        const saved = await saveGraphDraft(target.resourceRef, target.resourceKind);
        if (!isEditorCommandTargetCurrent(target)) return;
        if (!saved) {
          showBlockingMessage(
            t("notifications.project.saveFailed", { error: "graph_save_not_committed" }),
          );
        }
      } catch (e) {
        if (!isEditorCommandTargetCurrent(target)) return;
        logger.app.error(String(e), "ProjectOperations");
        showBlockingIpcError(e, "save_project_graph", (code) =>
          t("notifications.project.saveFailed", { error: code }),
        );
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

      const projectData = await loadActivatedProject(activation);
      if (!projectData) {
        showBlockingMessage(t("notifications.project.loadFailed"));
        return;
      }
    } catch (e) {
      logger.app.error(String(e), "ProjectOperations");
      showBlockingIpcError(
        e,
        "load_project_to_state",
        (code) => `${t("notifications.project.loadFailed")} (${code})`,
      );
    }
  }, [t]);

  const finalizeExecutionRun = useCallback(
    (graphPath: string, recording: RecordedEvent[], outcome: "success" | "cancelled" | "error") => {
      const store = useExecutionStore.getState();

      if (outcome === "cancelled") {
        store.interruptExecution(graphPath);
        return;
      }

      store.commitExecutionVisual(graphPath);

      if (recording.length > 0) {
        store.setRecording(graphPath, recording);
      }

      ensureGraphExecutionTerminal(graphPath, outcome === "error" ? "error" : "success");
    },
    [],
  );

  const compileGraph = useCallback(
    async (targetGraphPath?: string) => {
      const graphPath = resolveExecutionGraphPath(targetGraphPath);
      if (!graphPath) return false;
      try {
        return await compileGraphDraft(graphPath);
      } catch (error) {
        logger.exec.error(`编译失败: ${formatErrorMessage(error)}`);
        showBlockingIpcError(error, "compile_graph_draft", (code) =>
          t("notifications.project.compileFailed", { error: code }),
        );
        return false;
      }
    },
    [t],
  );

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

      try {
        const draft = useGraphDraftStore.getState().sessions[graphPath];
        if (draft?.compileStatus !== "compiled" || !draft.compiledArtifactId) {
          showBlockingMessage(t("notifications.project.compileRequired"));
          return;
        }
        const projection = useGraphProjectionStore.getState().graphEntities[graphPath];
        if (!isGraphProjectionExecutable(projection)) {
          showBlockingMessage(t("notifications.project.problemsBlockExecution"));
          return;
        }
        logger.exec.info(`执行当前 Analysis Graph: ${target.name} (${graphPath})`);

        const recording: RecordedEvent[] = [];
        const runState: GraphRunOutcomeState = { outcome: "success" };
        useExecutionStore.getState().startExecution(graphPath);

        await ProjectService.executeCompiledGraph({
          projectInstanceId: project.projectInstanceId,
          graphPath,
          compiledArtifactId: draft.compiledArtifactId,
          demand: { type: "default" },
          onEvent: (event) => {
            if (!isCurrentProjectIdentity(project)) return;
            observeGraphRunEvent(graphPath, event, runState);
            if (event.kind.type === "resultInspectionRequested") {
              void openInspectableResult(resultRef(event.kind.resultId), t);
            }
          },
          onOutput: (event) => {
            if (!isCurrentProjectIdentity(project)) return;
            observeGraphRunOutput(graphPath, event);
          },
        });

        if (!isCurrentProjectIdentity(project)) return;
        finalizeExecutionRun(graphPath, recording, runState.outcome);
      } catch (e) {
        if (!isCurrentProjectIdentity(project)) return;
        if (isExecutionCancelledError(e)) {
          logger.exec.info(`执行已中断: ${target.name} (${graphPath})`);
          finalizeExecutionRun(graphPath, [], "cancelled");
          return;
        }

        logger.exec.error(`执行失败: ${e instanceof Error ? e.message : String(e)}`);
        finalizeExecutionRun(graphPath, [], "error");
      }
    },
    [finalizeExecutionRun, t],
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
    compileGraph,
    executeGraph,
    cancelGraphExecution,
    clearGraphArtifacts,
  };
}
