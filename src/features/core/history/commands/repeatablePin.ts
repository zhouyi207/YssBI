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
  async execute(graphPath, args) {
    const result = await PinService.addRepeatablePin(graphPath, args.nodeId, args.slotIndex);
    return {
      nodeId: args.nodeId,
      slotIndex: args.slotIndex,
      pinId: result.pinId,
    };
  },

  async undo(graphPath, context) {
    await PinService.removeRepeatablePin(graphPath, context.nodeId, context.pinId);
  },

  async redo(graphPath, context) {
    const result = await PinService.addRepeatablePin(graphPath, context.nodeId, context.slotIndex);
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
  async execute(graphPath, args) {
    const result = await PinService.removeRepeatablePin(graphPath, args.nodeId, args.pinId);
    return {
      nodeId: args.nodeId,
      pinId: args.pinId,
      slotIndex: result.slotIndex,
      pinIndex: result.pinIndex,
    };
  },

  async undo(graphPath, context) {
    const result = await PinService.addRepeatablePin(graphPath, context.nodeId, context.slotIndex);
    context.pinId = result.pinId;
  },

  async redo(graphPath, context) {
    await PinService.removeRepeatablePin(graphPath, context.nodeId, context.pinId);
  },
};
