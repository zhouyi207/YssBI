export interface SequencedBatch {
  streamId: string;
  entries: readonly { sequence: number }[];
  failure?: "storage_unavailable";
}

export interface StreamWatermark {
  streamId: string;
  latestSequence: number;
}
const MAX_PENDING_BATCHES = 64;

export type RecordStreamDiscontinuity =
  | "preactivation-overflow"
  | "sequence-gap"
  | "storage-unavailable"
  | "invalid-batch";

export class RecordStreamDiscontinuityError extends Error {
  readonly reason: RecordStreamDiscontinuity;

  constructor(reason: RecordStreamDiscontinuity) {
    super(`Record stream requires reconnect: ${reason}`);
    this.name = "RecordStreamDiscontinuityError";
    this.reason = reason;
  }
}

export interface RecordBatchReceiver {
  onmessage: (value: unknown) => void;
  prepare: (snapshot: StreamWatermark) => RecordStreamDiscontinuity | null;
  activate: () => void;
  dispose: () => void;
  isDisposed: () => boolean;
}

interface SequenceInspection {
  streamId: string;
  watermark: number;
  gap: boolean;
}

function inspectSequence<Batch extends SequencedBatch>(
  batch: Batch,
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

export function createRecordBatchReceiver<Batch extends SequencedBatch>(
  parseBatch: (value: unknown) => Batch,
  onRecords: (batch: Batch) => void,
  onError: (error: unknown) => void = (error) => {
    console.error("[Records] Invalid or discontinuous channel batch", error);
  },
  maxPendingBatches = MAX_PENDING_BATCHES,
): RecordBatchReceiver {
  if (!Number.isInteger(maxPendingBatches) || maxPendingBatches <= 0) {
    throw new Error("Record pending batch capacity must be a positive integer");
  }

  let active = false;
  let disposed = false;
  let prepared = false;
  let pending: Batch[] = [];
  let streamId = "";
  let watermark = 0;
  let discontinuity: RecordStreamDiscontinuity | null = null;

  const deliver = (batch: Batch) => {
    try {
      onRecords(batch);
    } catch (error) {
      onError(error);
    }
  };

  const acceptPreparedBatch = (batch: Batch) => {
    const inspection = inspectSequence(batch, streamId, watermark);
    if (inspection.gap) {
      discontinuity = "sequence-gap";
      pending = [];
      const error = new RecordStreamDiscontinuityError("sequence-gap");
      onError(error);
      return true;
    }
    streamId = inspection.streamId;
    watermark = inspection.watermark;
    return false;
  };

  return {
    onmessage: (value) => {
      if (disposed || discontinuity) return;
      try {
        const batch = parseBatch(value);
        if (batch.failure) {
          discontinuity = "storage-unavailable";
          pending = [];
          onError(new RecordStreamDiscontinuityError(discontinuity));
          return;
        }
        if (active) {
          if (!acceptPreparedBatch(batch)) deliver(batch);
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
        discontinuity = "invalid-batch";
        pending = [];
        onError(
          error instanceof RecordStreamDiscontinuityError
            ? error
            : new RecordStreamDiscontinuityError("invalid-batch"),
        );
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
        throw new RecordStreamDiscontinuityError(discontinuity);
      }
      if (!prepared) {
        throw new Error("Record receiver must be prepared before activation");
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
