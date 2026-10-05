import {
  useExternalStoreRuntime,
  type AppendMessage,
  type ThreadMessageLike,
  MessageNotSentError,
} from "@assistant-ui/react";
import { useCallback, useEffect, useLayoutEffect, useRef } from "react";
import { useStore } from "zustand";
import { useAssistantModels } from "./assistantModels";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { AssistantHarnessProjection } from "./assistantHarnessSession";
import type { ProjectionMessage } from "./assistantHarnessProjection";

function convertProjectionMessage(message: ProjectionMessage): ThreadMessageLike {
  return {
    id: message.id,
    role: message.role,
    content: message.content,
    createdAt: message.createdAt,
    metadata: {
      custom: {
        finishedAt: message.finishedAt,
        updatedAt: message.updatedAt,
        model: message.model,
        resources: message.resources,
      },
    },
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

export function useAssistantHarnessRuntime(sessionId: string) {
  const projectionRef = useRef<AssistantHarnessProjection | null>(null);
  projectionRef.current ??= new AssistantHarnessProjection();
  const projection = projectionRef.current;
  const models = useAssistantModels();
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const snapshot = useStore(projection);
  useEffect(() => {
    void projection.start(sessionId);
    return projection.stop;
  }, [projection, projectInstanceId, sessionId]);
  useEffect(() => {
    projection.updateModels(models.catalog);
  }, [projection, models.catalog, snapshot.sessionId]);
  const submit = useCallback(
    async (message: AppendMessage) => {
      const sessionId = projection.getState().sessionId;
      const accepted = await projection.submit(appendedText(message));
      if (!accepted && sessionId === projection.getState().sessionId)
        throw new MessageNotSentError();
    },
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
  useLayoutEffect(() => {
    const key = snapshot.sessionId ? `yssbi.assistant.draft.${snapshot.sessionId}` : null;
    let draft = "";
    try {
      draft = key ? (localStorage.getItem(key) ?? "") : "";
    } catch {
      /* Storage may be unavailable. */
    }
    runtime.thread.composer.setText(draft);
    if (!key) return;
    let previous = draft;
    const persist = () => {
      const text = runtime.thread.composer.getState().text;
      if (text === previous) return;
      previous = text;
      try {
        if (text) localStorage.setItem(key, text);
        else localStorage.removeItem(key);
      } catch {
        /* Keep the current composer usable when local storage is full. */
      }
    };
    return runtime.thread.composer.subscribe(persist);
  }, [runtime, snapshot.sessionId]);
  return {
    runtime,
    snapshot,
    projection,
    selectModel: projection.selectModel,
    reconnect: useCallback(() => projection.start(sessionId), [projection, sessionId]),
    queueMessage: projection.queueMessage,
    removeQueuedMessage: projection.removeQueuedMessage,
    sendNextQueued: projection.sendNextQueued,
    restoreUnsentMessage: projection.restoreUnsentMessage,
    addResource: projection.addResource,
    removeResource: projection.removeResource,
    continueTask: projection.submit,
  };
}
