import { AssistantRuntimeProvider as AssistantUiRuntimeProvider } from "@assistant-ui/react";
import { createContext, useContext, useMemo, type PropsWithChildren } from "react";

import {
  useAssistantHarnessRuntime,
  type AssistantHarnessSnapshot,
} from "./assistantHarnessRuntime";

interface AssistantHarnessContextValue {
  readonly snapshot: AssistantHarnessSnapshot;
  readonly deleteMemory: (recordId: string) => Promise<void>;
  readonly newConversation: () => Promise<void>;
  readonly selectConversation: (sessionId: string) => Promise<void>;
  readonly reloadConversations: () => Promise<void>;
}

const AssistantHarnessContext = createContext<AssistantHarnessContextValue | null>(null);

export function AssistantRuntimeProvider({ children }: PropsWithChildren) {
  const {
    runtime,
    snapshot,
    deleteMemory,
    newConversation,
    selectConversation,
    reloadConversations,
  } = useAssistantHarnessRuntime();
  const context = useMemo(
    () => ({ snapshot, deleteMemory, newConversation, selectConversation, reloadConversations }),
    [snapshot, deleteMemory, newConversation, selectConversation, reloadConversations],
  );

  return (
    <AssistantHarnessContext value={context}>
      <AssistantUiRuntimeProvider runtime={runtime}>{children}</AssistantUiRuntimeProvider>
    </AssistantHarnessContext>
  );
}

export function useAssistantHarnessSnapshot(): AssistantHarnessSnapshot {
  const context = useContext(AssistantHarnessContext);
  if (!context) throw new Error("AssistantRuntimeProvider is missing");
  return context.snapshot;
}

export function useAssistantHarnessActions(): Omit<AssistantHarnessContextValue, "snapshot"> {
  const context = useContext(AssistantHarnessContext);
  if (!context) throw new Error("AssistantRuntimeProvider is missing");
  return {
    deleteMemory: context.deleteMemory,
    newConversation: context.newConversation,
    selectConversation: context.selectConversation,
    reloadConversations: context.reloadConversations,
  };
}
