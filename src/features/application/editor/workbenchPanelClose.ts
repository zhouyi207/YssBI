import i18n from "i18next";

import {
  canRemoveWorkbenchPanel,
  commitWorkbenchPanelRemoval,
  isWorkbenchPanelMetadata,
  releaseEditorPaneState,
  type EditorPanelMetadata,
  type EditorResourceKind,
  type WorkbenchPanelCommitToken,
  type WorkbenchPanelInfo,
  type WorkbenchPanelMetadata,
  workbenchLayoutRead,
} from "@/modules/workbench/public";
import { clearDetailFocusForClosedPanel } from "@/features/application/editor/clearDetailFocusForClosedPanel";
import { clearResourceDocumentState, isResourceDocumentDirty } from "@/features/core/resource";
import { resourceKey } from "@/features/core/resource/resourceTypes";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { uiStore } from "@/features/core/ui/UIStore";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { editorViewportScope, releaseEditorViewport } from "@/features/core/viewport";
import { logger } from "@/features/application/observability/appLogger";

import { saveChartDocument as saveChartDraft } from "@/features/application/chart/saveChartDocument";
import { deactivateGraphPanelSession } from "./graphPanelSession";
import { showBlockingIpcError, showBlockingMessage } from "./blockingErrorDialog";
import { unloadGraphDocument } from "./graphDocumentUnload";
import { resolveResourceDisplayName } from "./resolveResourceDisplayName";
import { saveGraph } from "@/features/application/graphEditing/saveGraph";
import { enqueueGraphTask } from "@/features/application/graphEditing/graphEditCoordinator";
import { useGraphEditingStore } from "@/features/core/graphEditing";
import type { GraphEditVersionDto } from "@/shared/types/domain/editorMutation";

type EditorDocument = {
  readonly key: string;
  readonly resourceRef: string;
  readonly resourceKind: EditorResourceKind;
  readonly name: string;
  readonly dirty: boolean;
  readonly version?: GraphEditVersionDto;
};

type CloseSnapshot = {
  readonly panels: readonly WorkbenchPanelInfo[];
  readonly allPanels: readonly WorkbenchPanelInfo[];
  readonly tokens: readonly WorkbenchPanelCommitToken[];
  readonly projectIdentity?: ProjectIdentitySnapshot;
};

type EditorPanelInfo = WorkbenchPanelInfo & { metadata: EditorPanelMetadata };

let closeWorkflowTail: Promise<void> = Promise.resolve();

function enqueueCloseWorkflow<T>(operation: () => Promise<T>): Promise<T> {
  const result = closeWorkflowTail.then(operation);
  closeWorkflowTail = result.then(
    () => undefined,
    () => undefined,
  );
  return result;
}

function cloneMetadata(metadata: WorkbenchPanelMetadata): WorkbenchPanelMetadata {
  return structuredClone(metadata);
}

function isCanonicalTarget(panel: WorkbenchPanelInfo): boolean {
  return (
    typeof panel.panelInstanceId === "string" &&
    panel.panelInstanceId.length > 0 &&
    typeof panel.groupId === "string" &&
    panel.groupId.length > 0 &&
    isWorkbenchPanelMetadata(panel.metadata)
  );
}

function isProjectScopedPanel(panel: WorkbenchPanelInfo): boolean {
  const metadata = panel.metadata;
  return metadata.role === "editor" || metadata.role === "result";
}

function captureCloseSnapshot(requestedPanelIds: readonly string[]): CloseSnapshot | null {
  const panelInstanceIds = [...new Set(requestedPanelIds)];
  if (panelInstanceIds.length === 0 || panelInstanceIds.some((id) => id.length === 0)) {
    return null;
  }

  const allPanels = [...workbenchLayoutRead.listPanels()];
  const panelsById = new Map<string, WorkbenchPanelInfo>();
  const duplicateIds = new Set<string>();
  for (const panel of allPanels) {
    if (panelsById.has(panel.panelInstanceId)) duplicateIds.add(panel.panelInstanceId);
    else panelsById.set(panel.panelInstanceId, panel);
  }

  const panels: WorkbenchPanelInfo[] = [];
  const tokens: WorkbenchPanelCommitToken[] = [];
  for (const panelInstanceId of panelInstanceIds) {
    const panel = panelsById.get(panelInstanceId);
    if (
      !panel ||
      duplicateIds.has(panelInstanceId) ||
      !isCanonicalTarget(panel) ||
      !canRemoveWorkbenchPanel(panel.metadata)
    )
      return null;
    const metadata = cloneMetadata(panel.metadata);
    panels.push({ ...panel, metadata });
    tokens.push({ panelInstanceId, groupId: panel.groupId, metadata });
  }

  if (!panels.some(isProjectScopedPanel)) return { panels, allPanels, tokens };
  try {
    return { panels, allPanels, tokens, projectIdentity: captureProjectIdentity() };
  } catch {
    return null;
  }
}

function editorKey(metadata: EditorPanelMetadata): string {
  return resourceKey({ id: metadata.resourceRef, kind: metadata.resourceKind });
}

function documentsThatLoseTheirLastPanel(snapshot: CloseSnapshot): EditorDocument[] {
  const closingIds = new Set(snapshot.panels.map((panel) => panel.panelInstanceId));
  const remainingKeys = new Set(
    snapshot.allPanels.flatMap((panel) => {
      const metadata = panel.metadata;
      return metadata.role === "editor" && !closingIds.has(panel.panelInstanceId)
        ? [editorKey(metadata)]
        : [];
    }),
  );
  const documents = new Map<string, EditorDocument>();

  for (const panel of snapshot.panels) {
    const metadata = panel.metadata;
    if (metadata.role !== "editor" || metadata.resourceKind === "database") continue;
    const key = editorKey(metadata);
    if (remainingKeys.has(key) || documents.has(key)) continue;
    const ref = { id: metadata.resourceRef, kind: metadata.resourceKind };
    documents.set(key, {
      key,
      resourceRef: metadata.resourceRef,
      resourceKind: metadata.resourceKind,
      name: resolveResourceDisplayName(ref, panel.title ?? metadata.resourceRef),
      dirty: isResourceDocumentDirty(ref),
      version:
        metadata.resourceKind === "chart"
          ? undefined
          : useGraphEditingStore.getState().sessions[metadata.resourceRef]?.version,
    });
  }
  return [...documents.values()];
}

function closeDialogOptions(document: EditorDocument) {
  return {
    title: i18n.t("editor.close.dirtyTitle"),
    message: i18n.t("editor.close.dirtyMessage", { name: document.name }),
    confirmText: i18n.t("editor.close.save"),
    discardText: i18n.t("editor.close.discard"),
    cancelText: i18n.t("editor.close.cancel"),
    type: "info" as const,
  };
}

async function saveChartDocument(
  document: EditorDocument,
  identity: ProjectIdentitySnapshot,
): Promise<boolean> {
  if (!isCurrentProjectIdentity(identity)) return false;
  try {
    const saved = await saveChartDraft(document.resourceRef);
    if (!isCurrentProjectIdentity(identity)) return false;
    if (saved) return true;
    showBlockingMessage(
      i18n.t("notifications.editor.documentSaveFailed", {
        title: document.name,
        error: "chart_save_not_committed",
      }),
    );
  } catch (error) {
    if (!isCurrentProjectIdentity(identity)) return false;
    showBlockingIpcError(error, "save_chart", (code) =>
      i18n.t("notifications.editor.documentSaveFailed", {
        title: document.name,
        error: code,
      }),
    );
  }
  return false;
}

async function saveGraphDocument(
  document: EditorDocument & { readonly resourceKind: "event" | "function" },
  identity: ProjectIdentitySnapshot,
): Promise<boolean> {
  if (!isCurrentProjectIdentity(identity)) return false;
  try {
    const saved = await saveGraph(document.resourceRef, document.resourceKind);
    return saved && isCurrentProjectIdentity(identity);
  } catch (error) {
    if (!isCurrentProjectIdentity(identity)) return false;
    showBlockingIpcError(error, "save_project_graph", (code) =>
      i18n.t("notifications.editor.documentSaveFailed", {
        title: document.name,
        error: code,
      }),
    );
    return false;
  }
}

function saveEditorDocument(
  document: EditorDocument,
  identity: ProjectIdentitySnapshot,
): Promise<boolean> {
  return document.resourceKind === "chart"
    ? saveChartDocument(document, identity)
    : saveGraphDocument(
        document as EditorDocument & { readonly resourceKind: "event" | "function" },
        identity,
      );
}

function isCloseSnapshotCurrent(snapshot: CloseSnapshot): boolean {
  return !snapshot.projectIdentity || isCurrentProjectIdentity(snapshot.projectIdentity);
}

function evictChartDocument(chartPath: string): void {
  useChartDocumentStore.setState((state) => {
    if (!Object.prototype.hasOwnProperty.call(state.documents, chartPath)) return {};
    const documents = { ...state.documents };
    delete documents[chartPath];
    return { documents };
  });
  clearResourceDocumentState({ id: chartPath, kind: "chart" });
}

function finalizeClosedPanels(
  snapshot: CloseSnapshot,
  closedPanels: readonly WorkbenchPanelInfo[] = snapshot.panels,
  discarded: ReadonlyMap<string, GraphEditVersionDto> = new Map(),
): void {
  const remainingEditors = workbenchLayoutRead
    .listPanels()
    .filter(
      (panel: WorkbenchPanelInfo): panel is EditorPanelInfo => panel.metadata.role === "editor",
    );
  const releasedViewportScopes = new Set<string>();
  const finalizedDocuments = new Set<string>();
  for (const panel of closedPanels) {
    const metadata = panel.metadata;
    if (metadata.role !== "editor") continue;
    releaseEditorPaneState(panel.panelInstanceId);

    if (metadata.resourceKind === "database") {
      if (
        !remainingEditors.some((candidate) => editorKey(candidate.metadata) === editorKey(metadata))
      )
        clearDetailFocusForClosedPanel(metadata.resourceRef);
      continue;
    }

    if (metadata.resourceKind !== "chart") {
      const hasSameScope = remainingEditors.some(
        (candidate: EditorPanelInfo) =>
          candidate.groupId === panel.groupId &&
          candidate.metadata.resourceRef === metadata.resourceRef,
      );
      const scopeKey = JSON.stringify([panel.groupId, metadata.resourceRef]);
      if (!hasSameScope && !releasedViewportScopes.has(scopeKey)) {
        releasedViewportScopes.add(scopeKey);
        releaseEditorViewport(editorViewportScope(panel.groupId, metadata.resourceRef));
        deactivateGraphPanelSession(panel.groupId, metadata.resourceRef);
      }
    }

    const key = editorKey(metadata);
    if (
      finalizedDocuments.has(key) ||
      remainingEditors.some((candidate: EditorPanelInfo) => editorKey(candidate.metadata) === key)
    ) {
      continue;
    }
    finalizedDocuments.add(key);
    clearDetailFocusForClosedPanel(metadata.resourceRef);
    if (metadata.resourceKind === "chart") {
      evictChartDocument(metadata.resourceRef);
      continue;
    }

    void unloadGraphDocument(metadata.resourceRef, discarded.get(metadata.resourceRef)).catch(
      () => {
        logger.graph.warn(
          "Failed to release graph cache after its last editor closed",
          "workbenchPanelClose",
        );
      },
    );
  }
}

function physicallyAbsentPanels(snapshot: CloseSnapshot): readonly WorkbenchPanelInfo[] {
  try {
    const liveIds = new Set(
      workbenchLayoutRead.listPanels().map((panel: WorkbenchPanelInfo) => panel.panelInstanceId),
    );
    return snapshot.panels.filter((panel) => !liveIds.has(panel.panelInstanceId));
  } catch {
    return [];
  }
}

function showCloseFailedMessage(): void {
  try {
    showBlockingMessage(i18n.t("editor.close.failed"));
  } catch {
    // The close promise must remain contained even if the feedback host is unavailable.
  }
}

export async function requestCloseWorkbenchPanel(panelInstanceId: string): Promise<boolean> {
  return requestCloseWorkbenchPanels([panelInstanceId]);
}

export function requestCloseWorkbenchGroup(groupId: string): Promise<boolean> {
  const panelInstanceIds = workbenchLayoutRead
    .listGroupPanels(groupId)
    .map((panel) => panel.panelInstanceId);
  return requestCloseWorkbenchPanels(panelInstanceIds);
}

async function requestCloseWorkbenchPanelsNow(
  panelInstanceIds: readonly string[],
): Promise<boolean> {
  let snapshot = captureCloseSnapshot(panelInstanceIds);
  if (!snapshot) return false;

  try {
    const graphs = documentsThatLoseTheirLastPanel(snapshot).filter(
      (document) => document.resourceKind !== "chart",
    );
    const settled = await Promise.all(
      graphs.map((document) => enqueueGraphTask(document.resourceRef, async () => true, false)),
    );
    if (settled.some((current) => !current) || !isCloseSnapshotCurrent(snapshot)) return false;
  } catch {
    showCloseFailedMessage();
    return false;
  }
  snapshot = captureCloseSnapshot(panelInstanceIds);
  if (!snapshot) return false;
  const discarded = new Map<string, GraphEditVersionDto>();

  for (const document of documentsThatLoseTheirLastPanel(snapshot)) {
    if (!document.dirty) continue;
    const decision = await uiStore.confirm3(closeDialogOptions(document));
    if (!isCloseSnapshotCurrent(snapshot) || decision === "cancel") return false;
    if (decision === "discard" && document.resourceKind !== "chart") {
      if (!document.version) return false;
      discarded.set(document.resourceRef, document.version);
    }
    if (decision === "confirm") {
      const identity = snapshot.projectIdentity;
      if (!identity || !(await saveEditorDocument(document, identity))) return false;
      if (!isCloseSnapshotCurrent(snapshot)) return false;
    }
  }

  let outcome: "committed" | "stale";
  try {
    const identity = snapshot.projectIdentity;
    outcome = identity
      ? await commitWorkbenchPanelRemoval(snapshot.tokens, () => isCurrentProjectIdentity(identity))
      : await commitWorkbenchPanelRemoval(snapshot.tokens);
  } catch {
    const absent = physicallyAbsentPanels(snapshot);
    if (isCloseSnapshotCurrent(snapshot) && absent.length > 0) {
      try {
        finalizeClosedPanels(snapshot, absent, discarded);
      } catch {
        // Physical removal already happened; never attempt a layout rollback here.
      }
    }
    showCloseFailedMessage();
    return false;
  }
  if (!isCloseSnapshotCurrent(snapshot) || outcome === "stale") return false;
  finalizeClosedPanels(snapshot, snapshot.panels, discarded);
  return true;
}

export function requestCloseWorkbenchPanels(panelInstanceIds: readonly string[]): Promise<boolean> {
  return enqueueCloseWorkflow(() => requestCloseWorkbenchPanelsNow(panelInstanceIds));
}
