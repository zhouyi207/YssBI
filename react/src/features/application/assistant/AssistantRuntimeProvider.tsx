import type { ResourceRef } from "@/shared/types/domain/resource";
import type { LanguageModelSelection } from "@/services/assistant/modelContract";
import type { HarnessTurnOptions } from "@/services/assistant/harnessContract";
import { AssistantRuntimeProvider as AssistantUiRuntimeProvider } from "@assistant-ui/react";
import { createContext, useContext, useMemo, type PropsWithChildren } from "react";
import { useStore } from "zustand";
import type { StoreApi } from "zustand/vanilla";

import { useAssistantHarnessRuntime } from "./assistantHarnessRuntime";
import type { AssistantHarnessSnapshot } from "./assistantHarnessProjection";

interface AssistantHarnessContextValue {
  readonly setTurnOptions: (options: HarnessTurnOptions) => void;
  readonly selectModel: (model: LanguageModelSelection) => Promise<void>;
  readonly projection: Pick<
    StoreApi<AssistantHarnessSnapshot>,
    "getState" | "getInitialState" | "subscribe"
  >;
  readonly reconnect: () => Promise<void>;
  readonly queueMessage: (text: string) => void;
  readonly addResource: (resource: ResourceRef) => void;
  readonly removeResource: (resource: ResourceRef) => void;
  readonly removeQueuedMessage: (id: string) => void;
  readonly sendNextQueued: () => Promise<void>;
  readonly restoreUnsentMessage: () => void;
  readonly continueTask: (text: string) => Promise<boolean>;
}

const AssistantHarnessContext = createContext<AssistantHarnessContextValue | null>(null);

export function AssistantRuntimeProvider({
  children,
  sessionId,
}: PropsWithChildren<{ sessionId: string }>) {
  const {
    runtime,
    projection,
    selectModel,
    setTurnOptions,
    reconnect,
    queueMessage,
    removeQueuedMessage,
    sendNextQueued,
    restoreUnsentMessage,
    addResource,
    removeResource,
    continueTask,
  } = useAssistantHarnessRuntime(sessionId);
  const context = useMemo(
    () => ({
      projection,
      selectModel,
      setTurnOptions,
      reconnect,
      queueMessage,
      removeQueuedMessage,
      sendNextQueued,
      restoreUnsentMessage,
      addResource,
      removeResource,
      continueTask,
    }),
    [
      projection,
      selectModel,
      setTurnOptions,
      reconnect,
      queueMessage,
      removeQueuedMessage,
      sendNextQueued,
      restoreUnsentMessage,
      addResource,
      removeResource,
      continueTask,
    ],
  );

  return (
    <AssistantHarnessContext value={context}>
      <AssistantUiRuntimeProvider runtime={runtime}>{children}</AssistantUiRuntimeProvider>
    </AssistantHarnessContext>
  );
}

export function useAssistantHarnessSnapshot<T>(
  selector: (snapshot: AssistantHarnessSnapshot) => T,
): T {
  const context = useContext(AssistantHarnessContext);
  if (!context) throw new Error("AssistantRuntimeProvider is missing");
  return useStore(context.projection, selector);
}

export function useAssistantHarnessActions(): Omit<AssistantHarnessContextValue, "projection"> {
  const context = useContext(AssistantHarnessContext);
  if (!context) throw new Error("AssistantRuntimeProvider is missing");
  return useMemo(
    () => ({
      selectModel: context.selectModel,
      setTurnOptions: context.setTurnOptions,
      reconnect: context.reconnect,
      queueMessage: context.queueMessage,
      removeQueuedMessage: context.removeQueuedMessage,
      sendNextQueued: context.sendNextQueued,
      restoreUnsentMessage: context.restoreUnsentMessage,
      addResource: context.addResource,
      removeResource: context.removeResource,
      continueTask: context.continueTask,
    }),
    [context],
  );
}
