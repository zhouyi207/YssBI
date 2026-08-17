import type { FrontendDiagnosticEntryDto } from '@/shared/types/dto/diagnostics';

export interface FrontendDiagnosticBatcherOptions {
  maxBatchEntries: number;
  maxPendingEntries: number;
  maxDelayMs: number;
  maxMessageBytes: number;
  submit: (entries: FrontendDiagnosticEntryDto[]) => Promise<void>;
}

export interface FrontendDiagnosticBatcher {
  enqueue: (entry: FrontendDiagnosticEntryDto) => void;
  flush: () => Promise<void>;
  dispose: () => void;
  pendingCount: () => number;
}

function positiveInteger(value: number, name: string): number {
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${name} must be a positive integer`);
  }
  return value;
}

function truncateUtf8(message: string, maxBytes: number): string {
  const encoder = new TextEncoder();
  if (encoder.encode(message).byteLength <= maxBytes) return message;

  const suffix = maxBytes >= 3 ? '…' : '.'.repeat(maxBytes);
  const contentBudget = maxBytes - encoder.encode(suffix).byteLength;
  let bytes = 0;
  let content = '';
  for (const character of message) {
    const characterBytes = encoder.encode(character).byteLength;
    if (bytes + characterBytes > contentBudget) break;
    bytes += characterBytes;
    content += character;
  }
  return `${content}${suffix}`;
}

export function createFrontendDiagnosticBatcher(
  options: FrontendDiagnosticBatcherOptions,
): FrontendDiagnosticBatcher {
  const maxBatchEntries = positiveInteger(options.maxBatchEntries, 'maxBatchEntries');
  const maxPendingEntries = positiveInteger(options.maxPendingEntries, 'maxPendingEntries');
  const maxDelayMs = positiveInteger(options.maxDelayMs, 'maxDelayMs');
  const maxMessageBytes = positiveInteger(options.maxMessageBytes, 'maxMessageBytes');

  let pending: FrontendDiagnosticEntryDto[] = [];
  let timer: ReturnType<typeof setTimeout> | null = null;
  let drain: Promise<void> | null = null;
  let disposed = false;

  const clearTimer = () => {
    if (timer === null) return;
    clearTimeout(timer);
    timer = null;
  };

  const startDrain = (): Promise<void> => {
    if (drain) return drain;
    clearTimer();
    drain = (async () => {
      while (!disposed && pending.length > 0) {
        const batch = pending.splice(0, maxBatchEntries);
        try {
          await options.submit(batch);
        } catch {
          // Diagnostics transport failures must not enter the logger again or retry forever.
        }
      }
    })().finally(() => {
      drain = null;
      if (disposed || pending.length === 0) return;
      if (pending.length >= maxBatchEntries) void startDrain();
      else scheduleDrain();
    });
    return drain;
  };

  const scheduleDrain = () => {
    if (timer !== null || drain !== null || disposed) return;
    timer = setTimeout(() => {
      timer = null;
      void startDrain();
    }, maxDelayMs);
  };

  return {
    enqueue: (entry) => {
      if (disposed) return;
      pending.push({
        ...entry,
        message: truncateUtf8(entry.message, maxMessageBytes),
      });
      if (pending.length > maxPendingEntries) {
        pending.splice(0, pending.length - maxPendingEntries);
      }
      if (pending.length >= maxBatchEntries) void startDrain();
      else scheduleDrain();
    },
    flush: async () => {
      if (disposed) return;
      clearTimer();
      await startDrain();
    },
    dispose: () => {
      disposed = true;
      clearTimer();
      pending = [];
    },
    pendingCount: () => pending.length,
  };
}
