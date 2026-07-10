import type { ReactNode } from "react";
import type { TFunction } from "i18next";

export type StatusBarAlignment = "left" | "right";

export interface StatusBarRenderContext {
  t: TFunction;
  projectStatus: string;
  projectFileName: string;
  activeTitle: string;
  activeType: string | null;
  activeTabId: string | null;
  selectedCount: number;
  nodeCount: number;
  connectionCount: number;
  executionStatus: string;
  themeMode: string;
}

export interface StatusBarItemViewModel {
  id: string;
  alignment: StatusBarAlignment;
  priority: number;
  content: ReactNode;
  tooltip?: string;
  onClick?: () => void;
  className?: string;
}

export interface StatusBarItemRegistration {
  id: string;
  alignment: StatusBarAlignment;
  /** Lower values appear first within the same side. */
  priority?: number;
  visible?: (ctx: StatusBarRenderContext) => boolean;
  render: (ctx: StatusBarRenderContext) => ReactNode;
  tooltip?: (ctx: StatusBarRenderContext) => string | undefined;
  onClick?: (ctx: StatusBarRenderContext) => void;
  className?: string | ((ctx: StatusBarRenderContext) => string | undefined);
}

export interface StatusBarItemsSnapshot {
  left: StatusBarItemViewModel[];
  right: StatusBarItemViewModel[];
}
