import { useEffect } from "react";
import { create } from "zustand";
import { HarnessService, type HarnessSession } from "@/services/assistant/harnessService";
import { toErrorReference, type ErrorReference } from "@/features/application/errorReference";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
  type ProjectLifecycleStateSnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { refreshActivityPanelDocument } from "@/features/application/sidebar/useActivityPanelDocument";

// Only transient mutation state lives here. Conversation content is the Rust Activity document.
const useMutations = create<{
  scope: ProjectLifecycleStateSnapshot;
  saving: boolean;
  error: ErrorReference | null;
  reopenSessionId: string | null;
}>(() => ({
  scope: captureProjectLifecycleState(),
  saving: false,
  error: null,
  reopenSessionId: null,
}));

function currentScope() {
  const scope = captureProjectLifecycleState();
  if (!isProjectLifecycleStateCurrent(useMutations.getState().scope))
    useMutations.setState({ scope, saving: false, error: null, reopenSessionId: null });
  return scope;
}

export const reloadAssistantConversations = () => refreshActivityPanelDocument("assistant");

// Remember only a closed conversation's identity; the Workbench Model still owns visibility.
export function rememberClosedAssistantConversation(sessionId: string | null): void {
  currentScope();
  useMutations.setState({ reopenSessionId: sessionId });
}

export function readClosedAssistantConversation(): string | null {
  currentScope();
  return useMutations.getState().reopenSessionId;
}

async function mutateConversation(
  action: () => Promise<HarnessSession>,
): Promise<HarnessSession | null> {
  const scope = currentScope();
  if (
    scope.projectInstanceId !== useProjectIOStore.getState().projectInstanceId ||
    useMutations.getState().saving
  )
    return null;
  useMutations.setState({ saving: true, error: null });
  try {
    const session = await action();
    if (!isProjectLifecycleStateCurrent(scope)) return null;
    await reloadAssistantConversations();
    return isProjectLifecycleStateCurrent(scope) ? session : null;
  } catch (error) {
    if (isProjectLifecycleStateCurrent(scope))
      useMutations.setState({ error: toErrorReference(error, "assistant_session_failed") });
    return null;
  } finally {
    if (isProjectLifecycleStateCurrent(scope)) useMutations.setState({ saving: false });
  }
}

export const createAssistantConversation = () =>
  mutateConversation(() => HarnessService.createSession());
export const renameAssistantConversation = (sessionId: string, title: string) =>
  mutateConversation(() => HarnessService.renameSession(sessionId, title.trim()));

export function useAssistantConversationMutations() {
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const state = useMutations();
  useEffect(() => {
    currentScope();
    const reload = () => {
      void reloadAssistantConversations();
    };
    window.addEventListener("focus", reload);
    return () => window.removeEventListener("focus", reload);
  }, [projectInstanceId]);
  return isProjectLifecycleStateCurrent(state.scope) ? state : { saving: false, error: null };
}
