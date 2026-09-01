import type { DockviewApi } from "dockview-react";
import { workbenchDockviewInternal } from "./workbenchDockviewInternal";

const workbenchBindingTokenBrand: unique symbol = Symbol("workbench-dockview-binding");

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
    const token: WorkbenchDockviewBindingToken = {
      [workbenchBindingTokenBrand]: generation,
    };
    return token;
  },
  unbind(token) {
    const generation = token[workbenchBindingTokenBrand];
    if (!current || current.generation !== generation) return;
    const bound = current;
    current = undefined;
    workbenchDockviewInternal.unbind(bound.api);
  },
};
