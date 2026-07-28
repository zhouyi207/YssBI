import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import type { CommandHandler } from '../types';
import { executeGraphIntent } from './executeGraphIntent';

export interface AddRepeatablePinArgs {
  nodeId: string;
  template: string;
}

export const addRepeatablePinCommand: CommandHandler<AddRepeatablePinArgs> = {
  execute(graphPath, args) {
    return executeGraphIntent(graphPath, {
      type: 'addPortInstance',
      payload: { nodeId: args.nodeId, template: args.template, order: null },
    });
  },
};

export interface RemoveRepeatablePinArgs {
  nodeId: string;
  pinId: string;
}

export const removeRepeatablePinCommand: CommandHandler<RemoveRepeatablePinArgs> = {
  execute(graphPath, args) {
    const pin = useGraphDataStore.getState().getGraphPin(graphPath, args.pinId);
    if (!pin?.address || pin.address.kind !== 'instance') {
      throw new Error(`Port '${args.pinId}' is not a removable port instance`);
    }
    if (pin.address.nodeId !== args.nodeId) {
      throw new Error(`Port '${args.pinId}' does not belong to node '${args.nodeId}'`);
    }
    return executeGraphIntent(graphPath, {
      type: 'removePortInstance',
      payload: { address: pin.address },
    });
  },
};
