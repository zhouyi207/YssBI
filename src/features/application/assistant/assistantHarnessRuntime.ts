import {
  useExternalStoreRuntime,
  type AppendMessage,
  type ThreadMessageLike,
} from "@assistant-ui/react";
import { useCallback, useEffect, useLayoutEffect, useRef } from "react";
import { useStore } from "zustand";
import { useSettingsRead } from "@/features/core/settings/read";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { AssistantHarnessProjection } from "./assistantHarnessSession";
import type { ProjectionMessage } from "./assistantHarnessProjection";

function convertProjectionMessage(message: ProjectionMessage): ThreadMessageLike {
  return {
    id: message.id,
    role: message.role,
    content: message.content,
    createdAt: message.createdAt,
    ...(message.role === "assistant" ? { status: message.status } : {}),
  };
}

function appendedText(message: AppendMessage): string {
  return message.content
    .filter(
      (part): part is Extract<(typeof message.content)[number], { type: "text" }> =>
        part.type === "text",
    )
    .map((part) => part.text)
    .join("\n")
    .trim();
}

export function useAssistantHarnessRuntime() {
  const projectionRef = useRef<AssistantHarnessProjection | null>(null);
  projectionRef.current ??= new AssistantHarnessProjection();
  const projection = projectionRef.current;
  const ai = useSettingsRead((state) => state.ai);
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const isLoading = useSettingsRead((state) => state.isLoading);
  const snapshot = useStore(projection);
  useEffect(() => {
    void projection.start();
    return projection.stop;
  }, [projection, projectInstanceId]);
  useEffect(() => {
    if (isLoading || !snapshot.sessionId) return;
    projection.invalidateProvider();
    const timer = window.setTimeout(() => {
      void projection.syncProvider(ai.openAiModel, ai.openAiBaseUrl, ai.openAiApiKey);
    }, 300);
    return () => {
      window.clearTimeout(timer);
      projection.invalidateProvider();
    };
  }, [
    ai.openAiApiKey,
    ai.openAiBaseUrl,
    ai.openAiModel,
    isLoading,
    projection,
    snapshot.sessionId,
  ]);
  const submit = useCallback(
    (message: AppendMessage) => projection.submit(appendedText(message)),
    [projection],
  );
  const runtime = useExternalStoreRuntime({
    messages: snapshot.messages,
    convertMessage: convertProjectionMessage,
    isRunning: snapshot.isRunning,
    isSendDisabled: projection.isSendDisabled(),
    onNew: submit,
    onCancel: projection.cancel,
  });
  const drafts = useRef(new Map<string, string>());
  const draftSessionId = useRef<string | null>(null);
  useLayoutEffect(() => {
    if (draftSessionId.current === snapshot.sessionId) return;
    if (draftSessionId.current)
      drafts.current.set(draftSessionId.current, runtime.thread.composer.getState().text);
    runtime.thread.composer.setText(
      snapshot.sessionId ? (drafts.current.get(snapshot.sessionId) ?? "") : "",
    );
    draftSessionId.current = snapshot.sessionId;
  }, [runtime, snapshot.sessionId]);
  return {
    runtime,
    snapshot,
    projection,
    deleteMemory: projection.deleteMemory,
    newConversation: projection.newConversation,
    selectConversation: projection.selectConversation,
    reloadConversations: projection.start,
  };
}
