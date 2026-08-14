import { useWorkbenchStore } from '@/features/core/workbench';

export function isZenModeActive(): boolean {
  return useWorkbenchStore.getState().zenMode;
}

export function enterZenMode(): void { useWorkbenchStore.getState().enterZenMode(); }
export function exitZenMode(): void { useWorkbenchStore.getState().exitZenMode(); }
export function clearZenModeSession(): void { useWorkbenchStore.getState().exitZenMode(); }
export function toggleZenMode(): void { useWorkbenchStore.getState().toggleZenMode(); }
