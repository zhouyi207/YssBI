import { Channel } from "@tauri-apps/api/core";
import { invokeCommand } from "@/services/ipc";
import { trackChannel, untrackChannel } from "@/services/devHmrIpc";
import { clearChannelMessageHandler } from "@/shared/platform/tauriWebview";
import {
  parseUiEvent,
  parseUiPage,
  parseUiUpdate,
  parseUiIntentReceipt,
} from "@/shared/types/dto/uiPresentation";
import type { ResultReference } from "@/shared/types/domain/result";
import type { UiAction, UiEvent, UiIntentStatus } from "@/shared/types/domain/uiPresentation";
import { isRecord } from "@/shared/types/report/guards";

export async function readUiPage(projectInstanceId: string, source: ResultReference) {
  const result = await invokeCommand<unknown>("inspect_ui", {
    projectInstanceId,
    request: { kind: "page", source },
  });
  if (!isRecord(result) || result.kind !== "page" || Object.keys(result).length !== 2)
    throw new Error("ui_spec_invalid");
  return parseUiPage(result.page);
}
export async function updateUiPage(
  projectInstanceId: string,
  source: ResultReference,
  baseRevision: number,
  action: UiAction,
) {
  return parseUiUpdate(
    await invokeCommand("update_ui", {
      projectInstanceId,
      request: { source, baseRevision, action },
    }),
  );
}
export async function activateUiElement(
  projectInstanceId: string,
  source: ResultReference,
  baseRevision: number,
  id: string,
) {
  return parseUiIntentReceipt(
    await invokeCommand("activate_ui_element", {
      projectInstanceId,
      request: { source, baseRevision, id, clientKey: crypto.randomUUID() },
    }),
  );
}
export async function pendingUiIntents(projectInstanceId: string) {
  const value = await invokeCommand<unknown>("pending_ui_intents", { projectInstanceId });
  if (!Array.isArray(value) || value.length > 128) throw new Error("ui_spec_invalid");
  return value.map(parseUiIntentReceipt);
}
export async function settleUiIntent(
  projectInstanceId: string,
  id: string,
  status: UiIntentStatus,
) {
  const value = await invokeCommand<unknown>("settle_ui_intent", { projectInstanceId, id, status });
  if (typeof value !== "boolean") throw new Error("ui_spec_invalid");
  return value;
}
async function openUiSubscription(
  projectInstanceId: string,
  workbench: boolean,
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
    if (id) void invokeCommand("unsubscribe_ui", { subscriptionId: id }).catch(onError);
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
    const result = await invokeCommand<unknown>("subscribe_ui", {
      projectInstanceId,
      workbench,
      channel,
    });
    if (typeof result !== "string" || !result) throw new Error("ui_spec_invalid");
    id = result;
    if (closed) await invokeCommand("unsubscribe_ui", { subscriptionId: id });
  } catch (error) {
    cleanup();
    throw error;
  }
  return async () => {
    if (closed) return;
    cleanup();
    if (id) await invokeCommand("unsubscribe_ui", { subscriptionId: id });
  };
}

type UiListener = {
  workbench: boolean;
  event: (event: UiEvent) => void;
  error: (error: unknown) => void;
};
type UiStream = {
  listeners: Set<UiListener>;
  close: (() => Promise<void>) | null;
  workbench: boolean;
  pending: Promise<void>;
};
const uiStreams = new Map<string, UiStream>();

/** One session stream per WebView/project; page hooks only own their listener. */
export async function subscribeUi(
  projectInstanceId: string,
  workbench: boolean,
  onEvent: (event: UiEvent) => void,
  onError: (error: unknown) => void,
) {
  let stream = uiStreams.get(projectInstanceId);
  if (!stream) {
    stream = { listeners: new Set(), close: null, workbench: false, pending: Promise.resolve() };
    uiStreams.set(projectInstanceId, stream);
  }
  const entry = stream;
  const alreadyConnected = entry.close !== null;
  const listener: UiListener = { workbench, event: onEvent, error: onError };
  entry.listeners.add(listener);
  const sync = () => {
    entry.pending = entry.pending
      .catch(() => {})
      .then(async () => {
        const wantsWorkbench = [...entry.listeners].some((value) => value.workbench);
        if (entry.close && (entry.listeners.size === 0 || wantsWorkbench !== entry.workbench)) {
          const close = entry.close;
          entry.close = null;
          await close();
        }
        if (entry.listeners.size === 0 || entry.close) return;
        entry.workbench = wantsWorkbench;
        entry.close = await openUiSubscription(
          projectInstanceId,
          wantsWorkbench,
          (event) => {
            const subscribers = Array.from(entry.listeners);
            for (const subscriber of subscribers) {
              try {
                subscriber.event(event);
              } catch (error) {
                subscriber.error(error);
              }
            }
          },
          (error) => {
            const subscribers = Array.from(entry.listeners);
            for (const subscriber of subscribers) subscriber.error(error);
          },
        );
      });
    return entry.pending;
  };
  try {
    await sync();
    // A late listener did not receive the stream's original ready notice.
    if (alreadyConnected && entry.listeners.has(listener)) onEvent({ kind: "resync" });
  } catch (error) {
    entry.listeners.delete(listener);
    if (entry.listeners.size === 0 && uiStreams.get(projectInstanceId) === entry)
      uiStreams.delete(projectInstanceId);
    throw error;
  }
  return async () => {
    entry.listeners.delete(listener);
    if (entry.listeners.size === 0 && uiStreams.get(projectInstanceId) === entry)
      uiStreams.delete(projectInstanceId);
    await sync();
  };
}
