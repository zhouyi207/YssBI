import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager';
import type { PlatformFailure, PlatformOutcome } from './platformTypes';

function operationFailure(operation: 'readClipboardText' | 'writeClipboardText'): PlatformFailure {
  return { operation, code: 'operationFailed' };
}

export async function readClipboardText(): Promise<PlatformOutcome<string>> {
  try {
    return { ok: true, value: await readText() };
  } catch {
    return { ok: false, failure: operationFailure('readClipboardText') };
  }
}

export async function writeClipboardText(value: string): Promise<PlatformOutcome<void>> {
  try {
    await writeText(value);
    return { ok: true, value: undefined };
  } catch {
    return { ok: false, failure: operationFailure('writeClipboardText') };
  }
}
