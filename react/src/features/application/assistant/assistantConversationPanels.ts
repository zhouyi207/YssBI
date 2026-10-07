import i18n from "i18next";
import {
  workbenchLayoutControl,
  workbenchLayoutRead,
  showWorkbenchLayoutError,
  revealWorkbenchView,
} from "@/modules/workbench/public";
import {
  requestCloseWorkbenchPanel,
  requestCloseWorkbenchPanels,
} from "@/features/application/editor/workbenchPanelClose";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { currentProjectionLocale } from "@/features/application/graphProjection/projectionLocale";
import {
  createAssistantConversation,
  reloadAssistantConversations,
  readClosedAssistantConversation,
  rememberClosedAssistantConversation,
} from "./assistantConversations";

export async function toggleAssistantConversationWindow(): Promise<void> {
  const scope = captureProjectLifecycleState();
  if (scope.projectInstanceId !== useProjectIOStore.getState().projectInstanceId) return;
  const panels = workbenchLayoutRead
    .listPanels()
    .filter((panel) => panel.metadata.role === "conversation");
  if (panels.length) {
    const selected = panels.find((panel) => panel.visible) ?? panels[0];
    const closed = await requestCloseWorkbenchPanels(panels.map((panel) => panel.panelInstanceId));
    if (
      closed &&
      isProjectLifecycleStateCurrent(scope) &&
      selected.metadata.role === "conversation"
    )
      rememberClosedAssistantConversation(selected.metadata.sessionId);
    return;
  }
  const binding = useSidebarStore.getState().bindPanel({
    ...scope,
    panelId: "assistant",
    locale: currentProjectionLocale(),
  });
  await reloadAssistantConversations();
  if (!isProjectLifecycleStateCurrent(scope)) return;
  const directory = useSidebarStore.getState().panels.assistant;
  if (directory?.binding !== binding) return;
  if (!directory.snapshot || directory.error) {
    await revealWorkbenchView("assistant");
    return;
  }
  const rows = directory.snapshot.document.rows;
  const previous = readClosedAssistantConversation();
  const conversation =
    rows.find(
      (row) =>
        row.kind === "item" && row.item.kind === "conversation" && row.item.sessionId === previous,
    ) ?? rows.find((row) => row.kind === "item" && row.item.kind === "conversation");
  if (conversation?.kind === "item" && conversation.item.kind === "conversation") {
    await openAssistantConversation(conversation.item.sessionId, false);
    return;
  }
  const created = await createAssistantConversation();
  if (!isProjectLifecycleStateCurrent(scope)) return;
  if (created) await openAssistantConversation(created.sessionId, false);
  else await revealWorkbenchView("assistant");
}

export async function closeAssistantConversation(sessionId: string): Promise<void> {
  const scope = captureProjectLifecycleState();
  if (scope.projectInstanceId !== useProjectIOStore.getState().projectInstanceId) return;
  const panel = workbenchLayoutRead
    .listPanels()
    .find(
      (panel) => panel.metadata.role === "conversation" && panel.metadata.sessionId === sessionId,
    );
  if (!panel) return;
  try {
    const closed = await requestCloseWorkbenchPanel(panel.panelInstanceId);
    if (closed && isProjectLifecycleStateCurrent(scope))
      rememberClosedAssistantConversation(sessionId);
  } catch (error) {
    showWorkbenchLayoutError(error);
  }
}

export async function openAssistantConversation(sessionId: string, toggle: boolean): Promise<void> {
  const directory = useSidebarStore.getState().panels.assistant;
  if (
    !directory ||
    directory.binding.projectInstanceId !== useProjectIOStore.getState().projectInstanceId ||
    !isProjectLifecycleStateCurrent(directory.binding)
  )
    return;
  const row = directory.snapshot?.document.rows.find(
    (row) =>
      row.kind === "item" && row.item.kind === "conversation" && row.item.sessionId === sessionId,
  );
  if (row?.kind !== "item" || row.item.kind !== "conversation") return;
  const session = row.item;
  const existing = workbenchLayoutRead
    .listPanels()
    .find(
      (panel) =>
        panel.metadata.role === "conversation" && panel.metadata.sessionId === session.sessionId,
    );
  try {
    if (toggle && existing?.visible) {
      await closeAssistantConversation(session.sessionId);
      return;
    }
    await workbenchLayoutControl.openConversation({
      sessionId: session.sessionId,
      title: session.title || i18n.t("panel.assistantNewConversation"),
    });
    if (isProjectLifecycleStateCurrent(directory.binding))
      rememberClosedAssistantConversation(null);
  } catch (error) {
    showWorkbenchLayoutError(error);
  }
}
