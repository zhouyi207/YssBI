import type { PinData } from '@/shared/types';
import type { GraphUndoPatch } from './graphUndoPatch';

export interface CreateNodeResult { nodeId: string; pinIds: string[]; }
export interface AutoDisconnected { fromPin: string; toPin: string; }
export interface ConnectPinsResult {
  fromPin: string;
  toPin: string;
  autoDisconnected: AutoDisconnected[];
}
export interface RemovedConnection { fromPin: string; toPin: string; }
export interface DisconnectPinResult {
  removedConnections: RemovedConnection[];
  undoPatch: GraphUndoPatch;
}
export interface AddRepeatablePinResult { pinId: string; pin: PinData; }
export interface RemoveRepeatablePinResult {
  removedPinId: string;
  slotIndex: number;
  pinIndex: number;
  removedConnections: [string, string][];
}

/** Item returned by the graph connection query command. */
export interface GraphConnectionQueryItem {
  id: string;
  from: string;
  to: string;
}

/** Item returned by the graph connection query command. */
export interface GraphConnectionQueryItem {
  id: string;
  from: string;
  to: string;
}
