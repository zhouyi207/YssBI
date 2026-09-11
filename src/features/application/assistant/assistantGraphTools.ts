import { getI18n } from "react-i18next";
import {
  HarnessGraphToolsService,
  type HarnessGraphToolRequest,
} from "@/services/assistant/harnessGraphToolsService";
import {
  enqueueGraphDraftTask,
  installDraftProjection,
} from "@/features/application/graphDraft/graphDraftCoordinator";
import { loadGraphProjection } from "@/features/application/graphProjection/graphProjectionLifecycle";
import { currentProjectionLocale } from "@/features/application/graphProjection/projectionLocale";
import { useGraphDraftStore } from "@/features/core/graphDraft";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  prepareGraphProjectionReplacements,
  commitPreparedGraphProjectionReplacements,
} from "@/features/core/dataStore/graphProjectionStore";
import { getGraphResourceKind } from "@/features/core/resource/resourceSelectors";
import { markResourceDirty, useResourceStore } from "@/features/core/resource";
import { useExecutionStore, ensureGraphExecutionTerminal } from "@/features/core/execution";
import {
  observeGraphRunEvent,
  observeGraphRunOutput,
} from "@/features/application/editor/observeGraphRunEvent";
import { openInspectableResult } from "@/features/application/execution/openInspectableResult";
import { resultRef } from "@/features/application/results";

export function assistantActiveGraphPath(): string | null {
  return useGraphSessionStore.getState().getFocusedGraphPath();
}

export async function applyAssistantGraphTool(
  request: HarnessGraphToolRequest,
  active: () => boolean,
): Promise<void> {
  let applied = false;
  try {
    const identity = captureProjectIdentity();
    if (!active() || identity.projectInstanceId !== request.projectInstanceId) return;
    if (
      !useGraphDraftStore.getState().sessions[request.graphPath] &&
      !(await loadGraphProjection(request.graphPath))
    )
      return;
    applied = await enqueueGraphDraftTask(
      request.graphPath,
      async () => {
        const session = useGraphDraftStore.getState().sessions[request.graphPath];
        if (!session || session.saving || !active() || !isCurrentProjectIdentity(identity))
          return false;
        const current = () =>
          active() &&
          isCurrentProjectIdentity(identity) &&
          useGraphDraftStore.getState().sessions[request.graphPath]?.sessionId ===
            session.sessionId &&
          useGraphDraftStore.getState().sessions[request.graphPath]?.draftGeneration ===
            session.draftGeneration;
        const drafts = useGraphDraftStore.getState();
        const saving = request.capabilityId === "save_graph";
        const compiling = request.capabilityId === "compile_graph";
        const running = request.capabilityId === "execute_graph";
        if (saving && !drafts.beginSave(request.graphPath)) return false;
        if (compiling && !drafts.beginCompile(request.graphPath)) return false;
        const compileRequest =
          useGraphDraftStore.getState().sessions[request.graphPath]?.compileRequest;
        const runState: { outcome: "success" | "error" | "cancelled" } = { outcome: "success" };
        if (running) useExecutionStore.getState().startExecution(request.graphPath);
        let completed = false;
        try {
          const update = await HarnessGraphToolsService.prepare(
            request,
            session.document,
            session.draftGeneration,
            currentProjectionLocale(),
            (event) => {
              if (!current()) return;
              observeGraphRunEvent(request.graphPath, event, runState);
              if (event.kind.type === "resultInspectionRequested")
                void openInspectableResult(
                  resultRef({
                    resultId: event.kind.resultId,
                    executionSessionId: event.run.executionSessionId,
                  }),
                  getI18n().t.bind(getI18n()),
                );
            },
            (event) => {
              if (current()) observeGraphRunOutput(request.graphPath, event);
            },
          );
          if (!current()) return false;
          if (!(await HarnessGraphToolsService.claim(request.requestId)) || !current())
            return false;
          switch (update.type) {
            case "draft": {
              if (useGraphDraftStore.getState().sessions[request.graphPath].saving) return false;
              if (update.update.changed) installDraftProjection(request.graphPath, update.update);
              else {
                const prepared = prepareGraphProjectionReplacements([
                  { graphPath: request.graphPath, projection: update.update.projection },
                ]);
                if (!prepared.prepared) return false;
                useGraphDraftStore
                  .getState()
                  .replaceResolvedProjection(request.graphPath, update.update.projection);
                commitPreparedGraphProjectionReplacements(prepared.plan);
              }
              break;
            }
            case "compilation": {
              if (
                !compileRequest ||
                !useGraphDraftStore.getState().isCompileCurrent(request.graphPath, compileRequest)
              )
                return false;
              const prepared = prepareGraphProjectionReplacements([
                { graphPath: request.graphPath, projection: update.update.projection },
              ]);
              if (!prepared.prepared) return false;
              commitPreparedGraphProjectionReplacements(prepared.plan);
              useGraphDraftStore
                .getState()
                .completeCompile(request.graphPath, update.update, compileRequest);
              break;
            }
            case "saved": {
              if (update.update.projectionReplacement.graphPath !== request.graphPath) return false;
              const prepared = prepareGraphProjectionReplacements([
                update.update.projectionReplacement,
              ]);
              if (!prepared.prepared) return false;
              useGraphDraftStore.getState().completeSave(request.graphPath, update.update);
              commitPreparedGraphProjectionReplacements(prepared.plan);
              const kind = getGraphResourceKind(request.graphPath);
              if (kind) {
                useResourceStore
                  .getState()
                  .patchResource(
                    { id: request.graphPath, kind },
                    { revision: update.update.resourceRevision },
                  );
                markResourceDirty({ id: request.graphPath, kind }, false);
              }
              break;
            }
            case "execution":
              ensureGraphExecutionTerminal(
                request.graphPath,
                update.update.status === "succeeded" ? "success" : "error",
              );
              break;
            case "none":
              break;
          }
          completed = true;
          return true;
        } finally {
          if (!completed && current()) {
            if (saving) useGraphDraftStore.getState().failSave(request.graphPath);
            if (compiling && compileRequest)
              useGraphDraftStore.getState().failCompile(request.graphPath, compileRequest);
            if (running) ensureGraphExecutionTerminal(request.graphPath, "error");
          }
        }
      },
      false,
    );
  } finally {
    await HarnessGraphToolsService.complete(request.requestId, applied);
  }
}

export async function subscribeAssistantGraphTools(
  sessionId: string,
): Promise<{ close(): Promise<void> }> {
  let active = true;
  const subscription = await HarnessGraphToolsService.subscribe(sessionId, (request) =>
    applyAssistantGraphTool(request, () => active),
  );
  return {
    close: async () => {
      active = false;
      await subscription.close();
    },
  };
}
