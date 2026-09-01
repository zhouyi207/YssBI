import type { ComponentType } from "react";

export interface WorkbenchSettingsOverlayProps {
  readonly onRequestClose: () => void;
}

export interface WorkbenchNodeDocumentationOverlayProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
}

export interface WorkbenchOverlayRegistry {
  readonly settings: ComponentType<WorkbenchSettingsOverlayProps>;
  readonly nodeDocumentation: ComponentType<WorkbenchNodeDocumentationOverlayProps>;
}
