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
  /** @deprecated use autoDisconnectedList */
  autoDisconnectedFrom: string | null;
  autoDisconnectedTo: string | null;
  autoDisconnectedList: AutoDisconnectedEntry[];
}

export const connectPinsCommand: CommandHandler<ConnectPinsArgs, ConnectPinsContext> = {
  async execute(_graphId, args) {
    const store = useGraphDataStore.getState();
    const draft = store.applyConnectionDraft(args.pinA, args.pinB, _graphId);
    const keys = draft ? [draft.connectionId, ...draft.disconnectedIds] : [];
    try {
      const result = await trackPending(
        CONNECTION_ECHO_DOMAIN,
        keys,
        ConnectionService.connectPins(_graphId, args.pinA, args.pinB),
      );

      return {
        pinA: args.pinA,
        pinB: args.pinB,
        fromPin: result.fromPin,
        toPin: result.toPin,
        autoDisconnectedFrom: result.autoDisconnectedFrom,
        autoDisconnectedTo: result.autoDisconnectedTo,
        autoDisconnectedList: result.autoDisconnected,
      };
    } catch (error) {
      if (draft) store.revertConnectionDraft(draft, _graphId);
      throw error;
    }
  },

  async undo(graphId, context) {
    const connectionId = `${context.fromPin}->${context.toPin}`;
    await ConnectionService.deleteConnection(graphId, connectionId);

    const toRestore = context.autoDisconnectedList ?? [];
    for (const entry of toRestore) {
      await ConnectionService.connectPins(graphId, entry.fromPin, entry.toPin);
    }
  },

  async redo(graphId, context) {
    await ConnectionService.connectPins(graphId, context.pinA, context.pinB);
  },
};
