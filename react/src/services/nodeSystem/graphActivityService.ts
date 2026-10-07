import { Channel } from "@tauri-apps/api/core";
import { invokeCommand } from "@/services/ipc";
import { trackChannel, untrackChannel } from "@/services/devHmrIpc";
import { clearChannelMessageHandler } from "@/shared/platform/tauriWebview";
import { isGraphResourcePath } from "@/shared/types/dto/editorProjectionGuards";
import { parseGraphEditingState } from "@/shared/types/dto/editorMutationWireParser";
import { parseRunEvent } from "@/shared/types/dto/runEventParser";
import type { GraphEditingStateDto } from "@/shared/types/domain/editorMutation";
import type { RunEvent } from "@/shared/types/domain/runEvent";

export interface GraphActivityObserver {
  changed(graphPath: string, editing: GraphEditingStateDto): void;
  execution(event: RunEvent): void;
  resync(): void;
  failed(error: unknown): void;
}

export async function subscribeGraphActivity(
  projectInstanceId: string,
  observer: GraphActivityObserver,
): Promise<{ close(): Promise<void> }> {
  let id: string | null = null;
  let closed = false;
  const cleanup = () => {
    closed = true;
    untrackChannel(channel);
    clearChannelMessageHandler(channel);
  };
  const channel = trackChannel(new Channel<unknown>(), () => {
    cleanup();
    if (id)
      void invokeCommand("unsubscribe_graph_activity", { subscriptionId: id }).catch(
        observer.failed,
      );
  });
  channel.onmessage = (raw) => {
    if (closed) return;
    try {
      if (!raw || typeof raw !== "object" || Array.isArray(raw))
        throw new Error("Invalid graph activity");
      const value = raw as Record<string, unknown>;
      if (value.projectInstanceId !== projectInstanceId)
        throw new Error("Graph activity project mismatch");
      switch (value.kind) {
        case "changed":
          if (Object.keys(value).length !== 4 || !isGraphResourcePath(value.graphPath))
            throw new Error("Invalid graph change");
          observer.changed(value.graphPath, parseGraphEditingState(value.editing));
          break;
        case "execution":
          if (Object.keys(value).length !== 3) throw new Error("Invalid graph run activity");
          observer.execution(parseRunEvent(value.event));
          break;
        case "resync":
          if (Object.keys(value).length !== 2) throw new Error("Invalid graph recovery notice");
          observer.resync();
          break;
        default:
          throw new Error("Unknown graph activity");
      }
    } catch (error) {
      observer.failed(error);
      observer.resync();
    }
  };
  try {
    const result = await invokeCommand("subscribe_graph_activity", {
      projectInstanceId,
      onEvent: channel,
    });
    if (typeof result !== "string" || !result)
      throw new Error("Invalid graph activity subscription");
    id = result;
    if (closed) await invokeCommand("unsubscribe_graph_activity", { subscriptionId: id });
  } catch (error) {
    cleanup();
    throw error;
  }
  return {
    close: async () => {
      if (closed) return;
      cleanup();
      if (id) await invokeCommand("unsubscribe_graph_activity", { subscriptionId: id });
    },
  };
}

export async function readExecutionSnapshot(projectInstanceId: string): Promise<RunEvent[]> {
  const value = await invokeCommand<unknown>("get_execution_snapshot", { projectInstanceId });
  if (!Array.isArray(value)) throw new Error("Invalid execution snapshot");
  return value.map(parseRunEvent);
}
