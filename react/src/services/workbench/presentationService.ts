import { Channel } from "@tauri-apps/api/core";
import { invokeCommand } from "@/services/ipc";
import { trackChannel, untrackChannel } from "@/services/devHmrIpc";
import { clearChannelMessageHandler } from "@/shared/platform/tauriWebview";
import { parseUiEvent, parseUiIntentReceipt } from "@/shared/types/dto/uiPresentation";
import type { UiEvent, UiIntentStatus } from "@/shared/types/domain/uiPresentation";

export async function pendingUiIntents(projectInstanceId: string) {
  const value = await invokeCommand<unknown>("pending_ui_intents", { projectInstanceId });
  if (!Array.isArray(value) || value.length > 128) throw new Error("ui_intent_invalid");
  return value.map(parseUiIntentReceipt);
}
export async function settleUiIntent(
  projectInstanceId: string,
  id: string,
  status: UiIntentStatus,
) {
  const value = await invokeCommand<unknown>("settle_ui_intent", { projectInstanceId, id, status });
  if (typeof value !== "boolean") throw new Error("ui_intent_invalid");
  return value;
}
export async function subscribeUiIntents(
  projectInstanceId: string,
  onEvent: (event: UiEvent) => void,
  onError: (error: unknown) => void,
) {
  let id: string | null = null;
  let closed = false;
  const cleanup = () => {
    closed = true;
    untrackChannel(channel);
    clearChannelMessageHandler(channel);
  };
  const channel = trackChannel(new Channel<unknown>(), () => {
    cleanup();
    if (id) void invokeCommand("unsubscribe_ui_intents", { subscriptionId: id }).catch(onError);
  });
  channel.onmessage = (raw) => {
    if (closed) return;
    try {
      onEvent(parseUiEvent(raw));
    } catch (error) {
      onError(error);
    }
  };
  try {
    const result = await invokeCommand<unknown>("subscribe_ui_intents", {
      projectInstanceId,
      channel,
    });
    if (typeof result !== "string" || !result) throw new Error("ui_intent_invalid");
    id = result;
    if (closed) await invokeCommand("unsubscribe_ui_intents", { subscriptionId: id });
  } catch (error) {
    cleanup();
    throw error;
  }
  return async () => {
    if (closed) return;
    cleanup();
    if (id) await invokeCommand("unsubscribe_ui_intents", { subscriptionId: id });
  };
}
