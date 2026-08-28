import type { DockviewApi } from 'dockview-react';
import { logsDockviewLayoutController } from './logsDockviewLayoutController';

declare const logsBindingTokenBrand: unique symbol;

export type LogsDockviewBindingToken = Readonly<{
  [logsBindingTokenBrand]: number;
}>;

let nextToken = 0;
let current: { readonly generation: number; readonly api: DockviewApi } | undefined;

export interface LogsDockviewRootBinding {
  bind(api: DockviewApi): LogsDockviewBindingToken;
  unbind(token: LogsDockviewBindingToken): void;
}

export const logsDockviewRootBinding: LogsDockviewRootBinding = {
  bind(api) {
    const generation = ++nextToken;
    current = { generation, api };
    logsDockviewLayoutController.bind(api);
    return { [logsBindingTokenBrand]: generation } as LogsDockviewBindingToken;
  },
  unbind(token) {
    const generation = token[logsBindingTokenBrand];
    if (!current || current.generation !== generation) return;
    const bound = current;
    current = undefined;
    logsDockviewLayoutController.unbind(bound.api);
  },
};
