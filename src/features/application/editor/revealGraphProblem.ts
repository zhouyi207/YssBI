import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useEditorStore } from "@/features/core/editor";
import { useResourceStore } from "@/features/core/resource";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { collectCanvasNodeWorldBounds } from "@/features/core/canvas";
import {
  commitViewport,
  editorViewportScope,
  fitWorldBounds,
  getViewport,
  persistGraphViewport,
  setViewportLive,
} from "@/features/core/viewport";
import { portAddressKey } from "@/features/domain/editorProjection";
import type { DiagnosticLocationDto } from "@/shared/types/domain/editorProjection";
import {
  updateEditorGroupSelectedConnectionIds,
  updateEditorGroupSelectedNodeIds,
  workbenchDockviewControl,
  workbenchDockviewRead,
} from "@/modules/workbench/public";
import { openGraphResource } from "./openGraphResource";
import { revealDetails } from "./rightSidebarActions";

export async function revealGraphProblem(
  graphPath: string,
  location: DiagnosticLocationDto,
  groupId: string,
): Promise<boolean> {
  const identity = captureProjectIdentity();
  const graph = useGraphProjectionStore.getState().graphEntities[graphPath];
  if (!graph) return false;
  if (location.kind === "resource") {
    const resource = Object.values(useResourceStore.getState().resources).find(
      (resource) =>
        resource.exists &&
        (resource.id === location.identity ||
          resource.uri === location.identity ||
          (resource.kind === "variable" && `variables/${resource.id}` === location.identity) ||
          (resource.kind === "database" && `databases/${resource.id}` === location.identity)),
    );
    if (!resource) return false;
    if (resource.kind === "event" || resource.kind === "function")
      await openGraphResource(resource.id, resource.kind);
    else if (resource.kind === "variable")
      await revealDetails({ kind: "variable", id: resource.id });
    else if (resource.kind === "database") await revealDetails({ kind: "data", id: resource.id });
    return isCurrentProjectIdentity(identity);
  }

  const nodeId =
    location.kind === "node" || location.kind === "parameter"
      ? location.nodeId
      : location.kind === "port"
        ? location.address.nodeId
        : null;
  if (nodeId && !graph.nodes[nodeId]) return false;
  if (nodeId) {
    updateEditorGroupSelectedNodeIds([nodeId], groupId);
    useEditorStore.getState().setDetailFocus({ kind: "node", id: nodeId, graphPath });
  }
  if (location.kind === "connection" && !graph.connections[location.connectionId]) return false;
  const panels = workbenchDockviewRead.findEditorPanelsByResource(graphPath);
  const panel = panels.find((panel) => panel.groupId === groupId) ?? panels[0];
  if (panel) {
    await workbenchDockviewControl.reveal(panel.panelInstanceId);
    const currentPanel = workbenchDockviewRead.getPanel(panel.panelInstanceId);
    if (
      !isCurrentProjectIdentity(identity) ||
      currentPanel?.metadata.role !== "editor" ||
      currentPanel.metadata.resourceRef !== graphPath
    )
      return false;
    groupId = currentPanel.groupId;
  }
  let nodeIds = nodeId ? [nodeId] : undefined;
  if (location.kind === "connection") {
    const connection = graph.connections[location.connectionId];
    if (!connection.output || !connection.input) return false;
    nodeIds = [connection.output.nodeId, connection.input.nodeId];
    updateEditorGroupSelectedConnectionIds([location.connectionId], groupId);
  } else if (nodeId) {
    updateEditorGroupSelectedNodeIds([nodeId], groupId);
  }

  if (nodeId) await revealDetails({ kind: "node", id: nodeId, graphPath });
  if (!panel) return true;
  if (location.kind !== "parameter") await workbenchDockviewControl.activate(panel.panelInstanceId);
  const revision = workbenchDockviewRead.getSnapshot().revision;
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  if (
    !isCurrentProjectIdentity(identity) ||
    workbenchDockviewRead.getSnapshot().revision !== revision
  )
    return false;
  const canvas = document.querySelector<HTMLElement>(
    `[data-editor-panel-instance-id="${CSS.escape(panel.panelInstanceId)}"]`,
  );
  if (canvas) {
    const scope = editorViewportScope(groupId, graphPath);
    const bounds = collectCanvasNodeWorldBounds({
      canvasElement: canvas,
      viewport: getViewport(scope),
      nodeIds,
    });
    if (bounds) {
      const rect = canvas.getBoundingClientRect();
      setViewportLive(scope, fitWorldBounds(bounds, { width: rect.width, height: rect.height }));
      commitViewport(scope);
      persistGraphViewport(scope);
    }
    if (location.kind === "port")
      canvas
        .querySelector<HTMLElement>(
          `[data-pin-id="${CSS.escape(portAddressKey(location.address))}"]`,
        )
        ?.focus({ preventScroll: true });
    else if (location.kind !== "parameter") canvas.focus({ preventScroll: true });
  }
  if (location.kind === "parameter") {
    const focus = useEditorStore.getState().detailFocus;
    if (focus?.kind !== "node" || focus.id !== nodeId || focus.graphPath !== graphPath)
      return false;
    const field = document.querySelector<HTMLElement>(
      `[data-graph-path="${CSS.escape(graphPath)}"][data-node-id="${CSS.escape(location.nodeId)}"][data-graph-parameter-key="${CSS.escape(location.key)}"]`,
    );
    field?.scrollIntoView({ block: "nearest" });
    (field?.querySelector<HTMLElement>("input, textarea, select, button") ?? field)?.focus();
  }
  return true;
}
