import { PinService } from '@/services/graph/pin/pinService';
import type { CommandHandler } from '../types';

export interface AddRepeatablePinArgs {
  nodeId: string;
  slotIndex: number;
}

export interface AddRepeatablePinContext {
  nodeId: string;
  slotIndex: number;
  pinId: string;
}

export const addRepeatablePinCommand: CommandHandler<AddRepeatablePinArgs, AddRepeatablePinContext> = {
  async execute(graphId, args) {
    const result = await PinService.addRepeatablePin(graphId, args.nodeId, args.slotIndex);
    return {
      nodeId: args.nodeId,
      slotIndex: args.slotIndex,
      pinId: result.pinId,
    };
  },

  async undo(graphId, context) {
    await PinService.removeRepeatablePin(graphId, context.nodeId, context.pinId);
  },

  async redo(graphId, context) {
    const result = await PinService.addRepeatablePin(graphId, context.nodeId, context.slotIndex);
    context.pinId = result.pinId;
  },
};

export interface RemoveRepeatablePinArgs {
  nodeId: string;
  pinId: string;
}

export interface RemoveRepeatablePinContext {
  nodeId: string;
  pinId: string;
  slotIndex: number;
  pinIndex: number;
}

export const removeRepeatablePinCommand: CommandHandler<RemoveRepeatablePinArgs, RemoveRepeatablePinContext> = {
  async execute(graphId, args) {
    const result = await PinService.removeRepeatablePin(graphId, args.nodeId, args.pinId);
    return {
      nodeId: args.nodeId,
      pinId: args.pinId,
      slotIndex: result.slotIndex,
      pinIndex: result.pinIndex,
    };
  },

  async undo(graphId, context) {
    const result = await PinService.addRepeatablePin(graphId, context.nodeId, context.slotIndex);
    context.pinId = result.pinId;
  },

  async redo(graphId, context) {
    await PinService.removeRepeatablePin(graphId, context.nodeId, context.pinId);
  },
};
