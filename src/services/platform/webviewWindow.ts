import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { PlatformFailure, PlatformOutcome } from './platformTypes';

export interface CreateWebviewWindowRequest {
  readonly label: string;
  readonly url: string;
  readonly title: string;
  readonly width: number;
  readonly height: number;
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
    new WebviewWindow(request.label, {
      url: request.url,
      title: request.title,
      width: request.width,
      height: request.height,
    });
    return { ok: true, value: undefined };
  } catch {
    return { ok: false, failure: operationFailure() };
  }
}
