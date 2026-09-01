import type { DiagnosticBatchDto, DiagnosticSubscriptionDto } from "@/shared/types/dto/diagnostics";
import { parseDiagnosticBatchDto } from "@/shared/types/dto/diagnosticsParser";

const MAX_PENDING_BATCHES = 64;

export type DiagnosticStreamDiscontinuity = "preactivation-overflow" | "sequence-gap";

export class DiagnosticStreamDiscontinuityError extends Error {
  readonly reason: DiagnosticStreamDiscontinuity;

  constructor(reason: DiagnosticStreamDiscontinuity) {
    super(`Diagnostic stream requires reconnect: ${reason}`);
    this.name = "DiagnosticStreamDiscontinuityError";
    this.reason = reason;
  }
}

export interface DiagnosticBatchReceiver {
  onmessage: (value: unknown) => void;
  prepare: (snapshot: DiagnosticSubscriptionDto) => DiagnosticStreamDiscontinuity | null;
  activate: () => void;
  dispose: () => void;
  isDisposed: () => boolean;
}

interface SequenceInspection {
  streamId: string;
  watermark: number;
  gap: boolean;
}

function inspectSequence(
  batch: DiagnosticBatchDto,
  streamId: string,
  watermark: number,
): SequenceInspection {
  const sequences = [...new Set(batch.entries.map((entry) => entry.sequence))]
    .filter((sequence) => batch.streamId !== streamId || sequence > watermark)
    .sort((left, right) => left - right);
  let nextWatermark = batch.streamId === streamId ? watermark : 0;
  let expected = batch.streamId === streamId ? watermark + 1 : 1;
  let gap = batch.streamId !== streamId;
  for (const sequence of sequences) {
    if (sequence !== expected) gap = true;
    nextWatermark = Math.max(nextWatermark, sequence);
    expected = sequence + 1;
  }
  return { streamId: batch.streamId, watermark: nextWatermark, gap };
}

export function createDiagnosticBatchReceiver(
  onRecords: (batch: DiagnosticBatchDto) => void,
  onError: (error: unknown) => void = (error) => {
    console.error("[Diagnostics] Invalid or discontinuous channel batch", error);
  },
  maxPendingBatches = MAX_PENDING_BATCHES,
): DiagnosticBatchReceiver {
  if (!Number.isInteger(maxPendingBatches) || maxPendingBatches <= 0) {
    throw new Error("Diagnostic pending batch capacity must be a positive integer");
  }

  let active = false;
  let disposed = false;
  let prepared = false;
  let pending: DiagnosticBatchDto[] = [];
  let streamId = "";
  let watermark = 0;
  let discontinuity: DiagnosticStreamDiscontinuity | null = null;

  const deliver = (batch: DiagnosticBatchDto) => {
    try {
      onRecords(batch);
    } catch (error) {
      onError(error);
    }
  };

  const acceptPreparedBatch = (batch: DiagnosticBatchDto) => {
    const inspection = inspectSequence(batch, streamId, watermark);
    streamId = inspection.streamId;
    watermark = inspection.watermark;
    if (inspection.gap) {
      const error = new DiagnosticStreamDiscontinuityError("sequence-gap");
      onError(error);
    }
    return inspection.gap;
  };

  return {
    onmessage: (value) => {
      if (disposed || discontinuity) return;
      try {
        const batch = parseDiagnosticBatchDto(value);
        if (active) {
          acceptPreparedBatch(batch);
          deliver(batch);
          return;
        }
        if (pending.length >= maxPendingBatches) {
          discontinuity = "preactivation-overflow";
          pending = [];
          return;
        }
        if (prepared && acceptPreparedBatch(batch)) {
          discontinuity = "sequence-gap";
          pending = [];
          return;
        }
        pending.push(batch);
      } catch (error) {
        onError(error);
      }
    },
    prepare: (snapshot) => {
      if (disposed) return "preactivation-overflow";
      if (discontinuity) return discontinuity;
      streamId = snapshot.streamId;
      watermark = snapshot.latestSequence;
      for (const batch of pending) {
        if (acceptPreparedBatch(batch)) {
          discontinuity = "sequence-gap";
          pending = [];
          return discontinuity;
        }
      }
      prepared = true;
      return null;
    },
    activate: () => {
      if (active || disposed) return;
      if (discontinuity) {
        throw new DiagnosticStreamDiscontinuityError(discontinuity);
      }
      if (!prepared) {
        throw new Error("Diagnostic receiver must be prepared before activation");
      }
      active = true;
      const queued = pending;
      pending = [];
      for (const batch of queued) deliver(batch);
    },
    dispose: () => {
      disposed = true;
      pending = [];
    },
    isDisposed: () => disposed,
  };
}
