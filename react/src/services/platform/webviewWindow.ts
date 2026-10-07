import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { PlatformFailure, PlatformOutcome, PlatformUnsubscribe } from "./platformTypes";

export interface CreateWebviewWindowRequest {
  readonly label: string;
  readonly url: string;
  readonly title: string;
  readonly width: number;
  readonly height: number;
  readonly decorations?: boolean;
  readonly visible?: boolean;
}

function invalidLabel(): PlatformFailure {
  return { operation: "createWebviewWindow", code: "invalidArgument", argument: "windowLabel" };
}

function invalidUrl(): PlatformFailure {
  return { operation: "createWebviewWindow", code: "invalidArgument", argument: "url" };
}

function operationFailure(): PlatformFailure {
  return { operation: "createWebviewWindow", code: "operationFailed" };
}

export async function createWebviewWindow(
  request: CreateWebviewWindowRequest,
): Promise<PlatformOutcome<void>> {
  if (request.label.trim().length === 0) return { ok: false, failure: invalidLabel() };
  if (request.url.trim().length === 0) return { ok: false, failure: invalidUrl() };
  try {
    const config: Record<string, unknown> = {
      url: request.url,
      title: request.title,
      width: request.width,
      height: request.height,
    };
    if (request.decorations !== undefined) config.decorations = request.decorations;
    if (request.visible !== undefined) config.visible = request.visible;
    const window = new WebviewWindow(request.label, config);
    return await new Promise<PlatformOutcome<void>>((resolve) => {
      let settled = false;
      const unlisteners: PlatformUnsubscribe[] = [];
      const finish = (outcome: PlatformOutcome<void>) => {
        if (settled) return;
        settled = true;
        for (const unlisten of unlisteners) unlisten();
        resolve(outcome);
      };
      const listen = (
        event: "tauri://created" | "tauri://error",
        outcome: PlatformOutcome<void>,
      ) => {
        void window
          .once(event, () => finish(outcome))
          .then(
            (unlisten) => {
              if (settled) unlisten();
              else unlisteners.push(unlisten);
            },
            () => finish({ ok: false, failure: operationFailure() }),
          );
      };
      listen("tauri://created", { ok: true, value: undefined });
      listen("tauri://error", { ok: false, failure: operationFailure() });
    });
  } catch {
    return { ok: false, failure: operationFailure() };
  }
}
