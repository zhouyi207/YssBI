import { Channel } from "@tauri-apps/api/core";
import { invokeCommand } from "./invokeCommand";
import { trackChannel, untrackChannel } from "@/services/devHmrIpc";
import { clearChannelMessageHandler } from "@/shared/platform/tauriWebview";
import {
  createRecordBatchReceiver,
  RecordStreamDiscontinuityError,
  type SequencedBatch,
  type StreamWatermark,
} from "./recordBatchReceiver";

const MAX_SUBSCRIPTION_ATTEMPTS = 2;

export interface RecordSubscription<Snapshot> {
  snapshot: Snapshot;
  activate: () => void;
  unsubscribe: () => Promise<void>;
}

export async function subscribeRecords<
  Batch extends SequencedBatch,
  Snapshot extends StreamWatermark & { subscriptionId: string },
>(
  contract: {
    subscribeCommand: string;
    unsubscribeCommand: string;
    parseSnapshot: (value: unknown) => Snapshot;
    parseBatch: (value: unknown) => Batch;
  },
  onRecords: (batch: Batch) => void,
  onDiscontinuity: (error: unknown) => void = () => {},
): Promise<RecordSubscription<Snapshot>> {
  const unsubscribeRemote = (subscriptionId: string) =>
    invokeCommand<void>(contract.unsubscribeCommand, { subscriptionId });
  for (let attempt = 0; attempt < MAX_SUBSCRIPTION_ATTEMPTS; attempt += 1) {
    let activated = false;
    let unsubscribed = false;
    const receiver = createRecordBatchReceiver(contract.parseBatch, onRecords, (error) => {
      if (!activated) return;
      cleanupChannel();
      if (subscriptionId && !unsubscribed) {
        unsubscribed = true;
        void unsubscribeRemote(subscriptionId).catch(() => {});
      }
      onDiscontinuity(error);
    });
    let subscriptionId: string | null = null;
    let hmrDisposed = false;
    let cleaned = false;
    let channel: Channel<unknown> | null = null;
    const cleanupChannel = () => {
      if (cleaned) return;
      cleaned = true;
      receiver.dispose();
      if (channel) {
        untrackChannel(channel);
        clearChannelMessageHandler(channel);
      }
    };
    channel = trackChannel(new Channel<unknown>(), () => {
      hmrDisposed = true;
      cleanupChannel();
      if (subscriptionId) {
        void unsubscribeRemote(subscriptionId).catch(() => {});
      }
    });
    channel.onmessage = receiver.onmessage;

    let snapshot: Snapshot;
    try {
      snapshot = contract.parseSnapshot(
        await invokeCommand(contract.subscribeCommand, { onRecords: channel }),
      );
      subscriptionId = snapshot.subscriptionId;
    } catch (error) {
      cleanupChannel();
      throw error;
    }

    if (hmrDisposed || receiver.isDisposed()) {
      await unsubscribeRemote(snapshot.subscriptionId).catch(() => {});
      throw new Error("Record subscription was disposed before activation");
    }

    const discontinuity = receiver.prepare(snapshot);
    if (discontinuity) {
      cleanupChannel();
      await unsubscribeRemote(snapshot.subscriptionId).catch(() => {});
      if (attempt + 1 < MAX_SUBSCRIPTION_ATTEMPTS) continue;
      throw new RecordStreamDiscontinuityError(discontinuity);
    }

    const unsubscribe = async () => {
      if (unsubscribed) return;
      unsubscribed = true;
      cleanupChannel();
      await unsubscribeRemote(snapshot.subscriptionId);
    };
    return {
      snapshot,
      activate: () => {
        try {
          activated = true;
          receiver.activate();
        } catch (error) {
          cleanupChannel();
          if (!unsubscribed) {
            unsubscribed = true;
            void unsubscribeRemote(snapshot.subscriptionId).catch(() => {});
          }
          throw error;
        }
      },
      unsubscribe,
    };
  }

  throw new Error("Record subscription attempts exhausted");
}
