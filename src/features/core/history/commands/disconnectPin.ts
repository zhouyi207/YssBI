import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import type { CommandHandler, GraphMutationCommandResult } from '../types';
import { executeGraphIntent } from './executeGraphIntent';

export interface DisconnectPortArgs {
  pinId: string;
}

export const disconnectPortCommand: CommandHandler<DisconnectPortArgs, GraphMutationCommandResult> = {
  execute(graphPath, args) {
    const pin = useGraphDataStore.getState().getGraphPin(graphPath, args.pinId);
    if (!pin?.address) return false;
    return executeGraphIntent(graphPath, {
      type: 'disconnectPort',
      payload: { address: pin.address },
    });
  },
};
