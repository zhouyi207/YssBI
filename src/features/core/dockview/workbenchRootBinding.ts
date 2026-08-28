import type { DockviewApi } from 'dockview-react';
import { workbenchDockviewInternal } from './workbenchDockviewInternal';
import { notifyWorkbenchRootBound, notifyWorkbenchRootUnbound } from './workbenchRead';

declare const workbenchBindingTokenBrand: unique symbol;

export type WorkbenchDockviewBindingToken = Readonly<{
  [workbenchBindingTokenBrand]: number;
}>;

let nextToken = 0;
let current: { readonly generation: number; readonly api: DockviewApi } | undefined;

export interface WorkbenchDockviewRootBinding {
  bind(api: DockviewApi): WorkbenchDockviewBindingToken;
  unbind(token: WorkbenchDockviewBindingToken): void;
}

export const workbenchDockviewRootBinding: WorkbenchDockviewRootBinding = {
  bind(api) {
    const generation = ++nextToken;
    current = { generation, api };
    workbenchDockviewInternal.bind(api);
    notifyWorkbenchRootBound();
    return { [workbenchBindingTokenBrand]: generation } as WorkbenchDockviewBindingToken;
  },
  unbind(token) {
    const generation = token[workbenchBindingTokenBrand];
    if (!current || current.generation !== generation) return;
    const bound = current;
    current = undefined;
    workbenchDockviewInternal.unbind(bound.api);
    notifyWorkbenchRootUnbound(generation);
  },
};
