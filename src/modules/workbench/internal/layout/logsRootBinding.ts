import { DockLocation, TabNode } from "flexlayout-react";
import { DEFAULT_LOGS_LAYOUT } from "./logsLayoutModel";
import { LayoutModelBinding } from "./layoutModelBinding";
import { logsLayoutRuntime } from "./logsRuntime";

const logsBindingTokenBrand: unique symbol = Symbol("logs-flexlayout-binding");

export type LogsLayoutBindingToken = Readonly<{
  [logsBindingTokenBrand]: number;
}>;

let nextToken = 0;
let current: { readonly generation: number; readonly api: LayoutModelBinding } | undefined;

export interface LogsLayoutRootBinding {
  create(): LayoutModelBinding;
  bind(api: LayoutModelBinding): LogsLayoutBindingToken;
  unbind(token: LogsLayoutBindingToken): void;
}

export const logsLayoutRootBinding: LogsLayoutRootBinding = {
  create: () =>
    new LayoutModelBinding(DEFAULT_LOGS_LAYOUT, (model) => {
      model.setOnAllowDrop(
        (source, drop) =>
          source instanceof TabNode &&
          drop.location === DockLocation.CENTER &&
          drop.node.getId() === source.getParent()?.getId(),
      );
    }),
  bind(api) {
    const generation = ++nextToken;
    current = { generation, api };
    logsLayoutRuntime.bind(api);
    const token: LogsLayoutBindingToken = {
      [logsBindingTokenBrand]: generation,
    };
    return token;
  },
  unbind(token) {
    const generation = token[logsBindingTokenBrand];
    if (!current || current.generation !== generation) return;
    const bound = current;
    current = undefined;
    logsLayoutRuntime.unbind(bound.api);
  },
};
