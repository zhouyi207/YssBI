import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { PlatformFailure, PlatformOutcome } from './platformTypes';

export interface CreateWebviewWindowRequest {
  readonly label: string;
  readonly url: string;
  readonly title: string;
  readonly width: number;
  readonly height: number;
  readonly x?: number;
  readonly y?: number;
  readonly decorations?: boolean;
  readonly visible?: boolean;
  readonly maximized?: boolean;
}

function invalidLabel(): PlatformFailure {
  return { operation: 'createWebviewWindow', code: 'invalidArgument', argument: 'windowLabel' };
}

function invalidUrl(): PlatformFailure {
  return { operation: 'createWebviewWindow', code: 'invalidArgument', argument: 'url' };
}

function operationFailure(): PlatformFailure {
  return { operation: 'createWebviewWindow', code: 'operationFailed' };
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
    if (typeof request.x === 'number' && typeof request.y === 'number') {
      config.x = request.x;
      config.y = request.y;
    }
    if (request.decorations !== undefined) config.decorations = request.decorations;
    if (request.visible !== undefined) config.visible = request.visible;
    if (request.maximized) config.maximized = true;
    new WebviewWindow(request.label, config);
    return { ok: true, value: undefined };
  } catch {
    return { ok: false, failure: operationFailure() };
  }
}
