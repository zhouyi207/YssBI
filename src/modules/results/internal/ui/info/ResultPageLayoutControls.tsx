import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { VscArrowDown, VscArrowUp } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { parseUiSpec, UI_SPEC_BYTE_LIMIT } from "@/shared/types/domain/uiPresentation";
import type { UiAction, UiElement, UiSpec } from "@/shared/types/domain/uiPresentation";

export function ResultPageLayoutControls({
  spec,
  busy,
  onAction,
}: {
  spec: UiSpec;
  busy: boolean;
  onAction: (action: UiAction) => Promise<boolean>;
}) {
  const { t } = useTranslation();
  const id = useId();
  const [text, setText] = useState("");
  const [invalid, setInvalid] = useState(false);
  const rows: { id: string; element: UiElement; index: number; count: number; depth: number }[] =
    [];
  const visit = (parent: string, depth: number) => {
    const children = spec.elements[parent].children;
    children.forEach((child, index) => {
      rows.push({ id: child, element: spec.elements[child], index, count: children.length, depth });
      visit(child, depth + 1);
    });
  };
  visit(spec.root, 0);
  return (
    <details className="mb-5 rounded-lg border border-border p-3">
      <summary className="cursor-pointer text-sm font-medium">{t("reportLayout.title")}</summary>
      <div className="mt-3 space-y-3">
        <p className="text-xs text-muted-foreground">{t("reportLayout.sessionOnly")}</p>
        <ul className="space-y-1">
          {rows.map(({ id: elementId, element, index, count, depth }) => {
            const label =
              element.component.type === "reportSection"
                ? t(`reportLayout.sections.${element.component.props.section}`)
                : elementId;
            return (
              <li
                key={elementId}
                className="flex items-center gap-2 rounded px-1 py-0.5"
                style={{ marginLeft: `${depth}rem` }}
              >
                <Checkbox
                  id={`${id}-${elementId}`}
                  checked={element.visible}
                  disabled={busy}
                  onCheckedChange={(checked) =>
                    void onAction({ kind: "visibility", id: elementId, visible: checked === true })
                  }
                />
                <label htmlFor={`${id}-${elementId}`} className="flex-1 cursor-pointer text-sm">
                  {label}
                </label>
                <Button
                  type="button"
                  variant="ghost"
                  size="xs"
                  aria-label={t("reportLayout.moveUp", { section: label })}
                  disabled={busy || index === 0}
                  onClick={() => void onAction({ kind: "move", id: elementId, offset: -1 })}
                >
                  <VscArrowUp aria-hidden />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="xs"
                  aria-label={t("reportLayout.moveDown", { section: label })}
                  disabled={busy || index === count - 1}
                  onClick={() => void onAction({ kind: "move", id: elementId, offset: 1 })}
                >
                  <VscArrowDown aria-hidden />
                </Button>
              </li>
            );
          })}
        </ul>
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() => void onAction({ kind: "reset" })}
        >
          {t("common.restoreDefaults")}
        </Button>
        <details className="border-t border-border pt-3">
          <summary className="cursor-pointer text-xs">{t("reportLayout.json")}</summary>
          <div className="mt-3 space-y-2">
            <label htmlFor={`${id}-json`} className="block text-xs text-muted-foreground">
              {t("reportLayout.jsonHelp")}
            </label>
            <textarea
              id={`${id}-json`}
              value={text}
              onChange={(event) => setText(event.target.value)}
              maxLength={UI_SPEC_BYTE_LIMIT + 1}
              spellCheck={false}
              rows={10}
              className="w-full rounded-md border border-input bg-background p-2 font-mono text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
              aria-invalid={invalid}
              aria-describedby={invalid ? `${id}-error` : undefined}
            />
            <div className="flex flex-wrap gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => {
                  setText(JSON.stringify(spec, null, 2));
                  setInvalid(false);
                }}
              >
                {t("reportLayout.export")}
              </Button>
              <Button
                type="button"
                size="sm"
                disabled={busy || !text.trim()}
                onClick={async () => {
                  try {
                    if (new TextEncoder().encode(text).length > UI_SPEC_BYTE_LIMIT)
                      throw new Error();
                    const candidate = parseUiSpec(JSON.parse(text));
                    setInvalid(false);
                    await onAction({ kind: "replace", spec: candidate });
                  } catch {
                    setInvalid(true);
                  }
                }}
              >
                {t("reportLayout.apply")}
              </Button>
            </div>
            {invalid && (
              <p id={`${id}-error`} role="alert" className="text-xs text-destructive">
                {t("reportLayout.errors.invalidShape", { path: "$" })}
              </p>
            )}
          </div>
        </details>
      </div>
    </details>
  );
}
