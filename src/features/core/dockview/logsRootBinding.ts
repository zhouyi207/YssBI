import type { DockviewApi } from 'dockview-react';
import { logsDockviewRuntime } from './logsRuntime';

const logsBindingTokenBrand: unique symbol = Symbol('logs-dockview-binding');

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
    logsDockviewRuntime.bind(api);
    const token: LogsDockviewBindingToken = {
      [logsBindingTokenBrand]: generation,
    };
    return token;
  },
  unbind(token) {
    const generation = token[logsBindingTokenBrand];
    if (!current || current.generation !== generation) return;
    const bound = current;
    current = undefined;
    logsDockviewRuntime.unbind(bound.api);
  },
};
