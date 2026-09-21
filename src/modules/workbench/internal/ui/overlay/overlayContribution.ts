import type { ComponentType } from "react";

export interface WorkbenchNodeDocumentationOverlayProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
}

export interface WorkbenchOverlayRegistry {
  readonly nodeDocumentation: ComponentType<WorkbenchNodeDocumentationOverlayProps>;
}
