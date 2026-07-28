export type CommandType =
  | 'MoveNodes'
  | 'SetPinValue'
  | 'ConnectPins'
  | 'DisconnectPin'
  | 'DeleteNodes'
  | 'AddRepeatablePin'
  | 'RemoveRepeatablePin';

export interface CommandHandler<TArgs = unknown, TResult = unknown> {
  execute(graphPath: string, args: TArgs): Promise<TResult> | TResult;
}
