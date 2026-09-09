import type { NodeCreationDescriptorDto } from "./nodeCreationDescriptor";

export type ActivityPanelId = "project" | "nodes" | "commands" | "plugins";
export type BackendActivityPanelId = Exclude<ActivityPanelId, "project">;
export type ActivityText = { readonly key: string } | { readonly text: string };
export type ActivityActionId =
  | "newEvent"
  | "newFunction"
  | "newChart"
  | "importData"
  | "install"
  | "refresh";
export interface ActivityTool {
  readonly id: ActivityActionId;
  readonly label: ActivityText;
  readonly icon: "add" | "install" | "refresh";
}
export type ActivityItem =
  | {
      readonly kind: "graph";
      readonly path: string;
      readonly name: string;
      readonly graphType: "event" | "function";
    }
  | { readonly kind: "chart"; readonly path: string; readonly name: string }
  | {
      readonly kind: "database";
      readonly id: string;
      readonly resourcePath: string;
      readonly name: string;
    }
  | {
      readonly kind: "node";
      readonly key: string;
      readonly title: string;
      readonly creation: NodeCreationDescriptorDto;
    }
  | { readonly kind: "command"; readonly id: "undo" | "redo"; readonly label: ActivityText }
  | {
      readonly kind: "plugin";
      readonly id: string;
      readonly name: string;
      readonly description: string;
      readonly publisher: string;
      readonly enabled: boolean;
    };
export type ActivityPanelRow = { readonly id: string; readonly depth: number } & (
  | {
      readonly kind: "category";
      readonly label: ActivityText;
      readonly defaultExpanded: boolean;
      readonly tools: readonly ActivityTool[];
      readonly count: number | null;
    }
  | { readonly kind: "item"; readonly item: ActivityItem }
  | {
      readonly kind: "message";
      readonly label: ActivityText;
      readonly description: ActivityText | null;
    }
);
export interface ActivityPanelDocument {
  readonly schema: "yssbi.activity-panel.v1";
  readonly panelId: ActivityPanelId;
  readonly projectInstanceId: string | null;
  readonly publicationRevision: number;
  readonly title: ActivityText;
  readonly tools: readonly ActivityTool[];
  /** Complete ordered tree; collapsing filters visibility without changing this projection. */
  readonly rows: readonly ActivityPanelRow[];
  readonly emptyState: { readonly title: ActivityText; readonly description: ActivityText } | null;
}

/** The cursor identifies a Rust delivery baseline; it is not the project publication revision. */
export interface ActivityPanelSnapshot {
  readonly cursor: string;
  readonly document: ActivityPanelDocument;
}
