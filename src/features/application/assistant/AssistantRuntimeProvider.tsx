import { AssistantRuntimeProvider as AssistantUiRuntimeProvider } from "@assistant-ui/react";
import { createContext, useContext, useMemo, type PropsWithChildren } from "react";
import { useStore } from "zustand";
import type { StoreApi } from "zustand/vanilla";

import { useAssistantHarnessRuntime } from "./assistantHarnessRuntime";
import type { AssistantHarnessSnapshot } from "./assistantHarnessProjection";

interface AssistantHarnessContextValue {
  readonly projection: Pick<
    StoreApi<AssistantHarnessSnapshot>,
    "getState" | "getInitialState" | "subscribe"
  >;
  readonly deleteMemory: (recordId: string) => Promise<void>;
  readonly newConversation: () => Promise<void>;
  readonly selectConversation: (sessionId: string) => Promise<void>;
  readonly reloadConversations: () => Promise<void>;
}

const AssistantHarnessContext = createContext<AssistantHarnessContextValue | null>(null);

export function AssistantRuntimeProvider({ children }: PropsWithChildren) {
  const {
    runtime,
    projection,
    deleteMemory,
    newConversation,
    selectConversation,
    reloadConversations,
  } = useAssistantHarnessRuntime();
  const context = useMemo(
    () => ({ projection, deleteMemory, newConversation, selectConversation, reloadConversations }),
    [projection, deleteMemory, newConversation, selectConversation, reloadConversations],
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
      deleteMemory: context.deleteMemory,
      newConversation: context.newConversation,
      selectConversation: context.selectConversation,
      reloadConversations: context.reloadConversations,
    }),
    [context],
  );
}
