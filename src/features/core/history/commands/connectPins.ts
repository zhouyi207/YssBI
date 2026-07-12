import { ConnectionService } from '@/services';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { trackPending } from '@/features/core/sync/utils/echoSuppressor';
import type { CommandHandler } from '../types';

/** Echo 抑制域：connect 期间后端回传的 ConnectionCreated/Deleted 事件 key */
export const CONNECTION_ECHO_DOMAIN = 'connection';

export interface ConnectPinsArgs {
  pinA: string;
  pinB: string;
}

interface AutoDisconnectedEntry {
  fromPin: string;
  toPin: string;
}

export interface ConnectPinsContext {
  pinA: string;
  pinB: string;
  fromPin: string;
  toPin: string;
  autoDisconnectedList: AutoDisconnectedEntry[];
}

export const connectPinsCommand: CommandHandler<ConnectPinsArgs, ConnectPinsContext> = {
  async execute(_graphPath, args) {
    const store = useGraphDataStore.getState();
    const draft = store.applyConnectionDraft(args.pinA, args.pinB, _graphPath);
    const keys = draft ? [draft.connectionId, ...draft.disconnectedIds] : [];
    try {
      const result = await trackPending(
        CONNECTION_ECHO_DOMAIN,
        keys,
        ConnectionService.connectPins(_graphPath, args.pinA, args.pinB),
      );

      return {
        pinA: args.pinA,
        pinB: args.pinB,
        fromPin: result.fromPin,
        toPin: result.toPin,
        autoDisconnectedList: result.autoDisconnected,
      };
    } catch (error) {
      if (draft) store.revertConnectionDraft(draft, _graphPath);
      throw error;
    }
  },

  async undo(graphPath, context) {
    const connectionId = `${context.fromPin}->${context.toPin}`;
    await ConnectionService.deleteConnection(graphPath, connectionId);

    for (const entry of context.autoDisconnectedList) {
      await ConnectionService.connectPins(graphPath, entry.fromPin, entry.toPin);
    }
  },

  async redo(graphPath, context) {
    await ConnectionService.connectPins(graphPath, context.pinA, context.pinB);
  },
};
