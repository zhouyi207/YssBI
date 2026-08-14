import type { PinData } from '@/shared/types';


export interface CreateNodeResult { nodeId: string; pinIds: string[]; }
export interface AutoDisconnected { fromPin: string; toPin: string; }
export interface ConnectPinsResult {
  fromPin: string;
  toPin: string;
  autoDisconnected: AutoDisconnected[];
}

export interface AddRepeatablePinResult { pinId: string; pin: PinData; }
export interface RemoveRepeatablePinResult {
  removedPinId: string;
  slotIndex: number;
  pinIndex: number;
  removedConnections: [string, string][];
}