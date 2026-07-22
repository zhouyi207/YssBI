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
  activeEditorGroupId: string | null;
  selectedCount: number;
  nodeCount: number;
  connectionCount: number;
  executionStatus: string;
  colorTheme: string;
  juliaWorkerState: "checking" | "starting" | "ready" | "unavailable";
  juliaWorkerLabel: string;
  juliaWorkerTooltip: string;
}

export interface StatusBarItemViewModel {
  id: string;
  alignment: StatusBarAlignment;
  priority: number;
  content: ReactNode;
  /** Accessible name for interactive items; falls back to tooltip when omitted. */
  ariaLabel?: string;
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
  ariaLabel?: (ctx: StatusBarRenderContext) => string | undefined;
  tooltip?: (ctx: StatusBarRenderContext) => string | undefined;
  onClick?: (ctx: StatusBarRenderContext) => void;
  className?: string | ((ctx: StatusBarRenderContext) => string | undefined);
}

export interface StatusBarItemsSnapshot {
  left: StatusBarItemViewModel[];
  right: StatusBarItemViewModel[];
}
