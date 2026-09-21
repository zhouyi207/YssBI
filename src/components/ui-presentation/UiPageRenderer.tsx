import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import type { ReportSectionKind, UiSpec } from "@/shared/types/domain/uiPresentation";

export function UiPageRenderer({
  spec,
  renderReportSection,
  onActivate,
  disabled,
}: {
  spec: UiSpec;
  renderReportSection: (kind: ReportSectionKind) => ReactNode;
  onActivate: (id: string) => void;
  disabled: boolean;
}) {
  const render = (id: string): ReactNode => {
    const element = spec.elements[id];
    if (!element.visible) return null;
    const component = element.component;
    switch (component.type) {
      case "column":
      case "row":
        return (
          <div
            key={id}
            data-ui-element={id}
            className="flex min-w-0"
            style={{
              flexDirection: component.type === "row" ? "row" : "column",
              flexWrap: component.type === "row" ? "wrap" : undefined,
              gap: `${component.props.gap * 0.25}rem`,
            }}
          >
            {element.children.map(render)}
          </div>
        );
      case "text":
        return (
          <p key={id} data-ui-element={id} className="whitespace-pre-wrap text-sm">
            {component.props.text}
          </p>
        );
      case "reportSection":
        return (
          <div key={id} data-ui-element={id} className="min-w-0 flex-1">
            {renderReportSection(component.props.section)}
          </div>
        );
      case "button":
        return (
          <Button
            key={id}
            data-ui-element={id}
            type="button"
            disabled={disabled}
            onClick={() => onActivate(id)}
          >
            {component.props.label}
          </Button>
        );
    }
  };
  return render(spec.root);
}
