import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import type { UiBindingKind, UiBoundComponent, UiSpec } from "@/shared/types/domain/uiPresentation";
import type { UiDisplayData } from "@/shared/types/domain/uiData";
import { Section } from "./Section";
import { KeyValue } from "./KeyValue";
import { DataTable } from "./DataTable";
import { StatCard } from "./StatCard";

export type UiResultBindings = Readonly<
  Record<
    string,
    {
      readonly type: Exclude<UiBindingKind, "keyValue" | "statCard">;
      readonly content: ReactNode;
      readonly available?: boolean;
    }
  >
>;

export function UiPageRenderer({
  spec,
  data,
  results,
  onActivate,
  disabled,
}: {
  spec: UiSpec;
  data: Readonly<Record<string, UiDisplayData>>;
  results: UiResultBindings;
  onActivate: (id: string) => void;
  disabled: boolean;
}) {
  const hasOwn = (value: object, key: string) => Object.prototype.hasOwnProperty.call(value, key);
  const available = (id: string): boolean => {
    const element = spec.elements[id];
    if (!element.visible) return false;
    if ("binding" in element.component.props) {
      const binding = element.component.props.binding;
      return !hasOwn(results, binding) || results[binding].available !== false;
    }
    if (["section", "row", "column"].includes(element.component.type))
      return element.children.some(available);
    return true;
  };
  const renderBinding = (component: UiBoundComponent): ReactNode => {
    const binding = component.props.binding;
    if (hasOwn(data, binding)) {
      const value = data[binding];
      if (component.type !== value.kind) throw new Error("ui_binding_invalid");
      switch (value.kind) {
        case "keyValue":
          return <KeyValue items={value.items} />;
        case "table":
          return <DataTable columns={value.columns} rows={value.rows} />;
        case "statCard":
          return <StatCard {...value.stat} />;
      }
    }
    if (!hasOwn(results, binding) || results[binding].type !== component.type)
      throw new Error("ui_binding_invalid");
    return results[binding].content;
  };
  const render = (id: string): ReactNode => {
    if (!available(id)) return null;
    const element = spec.elements[id];
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
      case "section":
        return (
          <div key={id} data-ui-element={id} className="min-w-0 flex-1">
            <Section {...component.props}>{element.children.map(render)}</Section>
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
      default:
        return (
          <div key={id} data-ui-element={id} className="min-w-0 flex-1">
            {renderBinding(component)}
          </div>
        );
    }
  };
  return render(spec.root);
}
