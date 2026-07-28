import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import type { CommandHandler } from '../types';
import { executeGraphIntent } from './executeGraphIntent';

export interface DisconnectPinArgs {
  pinId: string;
}

export const disconnectPinCommand: CommandHandler<DisconnectPinArgs, boolean> = {
  async execute(graphPath, args) {
    const connectionIds = [
      ...useGraphDataStore.getState().getGraphPinConnections(graphPath, args.pinId),
    ];
    if (connectionIds.length === 0) return false;

    for (const connectionId of connectionIds) {
      const outcome = await executeGraphIntent(graphPath, {
        type: 'disconnect',
        payload: { connectionId },
      });
      if (outcome.status !== 'applied') return false;
    }
    return true;
  },
};
